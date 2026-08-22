use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Semaphore;
use log::{info, warn, debug};
use std::collections::HashMap;

use crate::collector::driver::{OltDriver, OltTarget, OnuOpticalData};
use crate::collector::snmp::SnmpClient;

/// Driver Parks para OLTs GPON Fiberlink (Fiberlink 30028 / 21000 / 21016 / 21004 / 21008)
pub struct ParksDriver;

impl ParksDriver {
    pub fn new() -> Self {
        Self
    }

    /// Decodifica o índice de slot, pon_port e onu_id da MIB Parks (Enterprise 6771)
    /// OID típico: .1.3.6.1.4.1.6771.10.1.5.1.18.<slot>.<port>.<onu_id>
    fn parse_parks_index(oid: &str) -> Option<(i32, i32, i32)> {
        let parts: Vec<&str> = oid.trim_start_matches('.').split('.').collect();
        if parts.len() >= 3 {
            let onu_id = parts[parts.len() - 1].parse::<i32>().ok()?;
            let port = parts[parts.len() - 2].parse::<i32>().ok()?;
            let slot = parts[parts.len() - 3].parse::<i32>().ok()?;
            return Some((slot, port, onu_id));
        }
        None
    }

    /// Decodifica o serial codificado em hexadecimal da Parks (ex: "50524B5300D6F760" -> "PRKS00D6F760")
    fn decode_parks_serial(raw: &str) -> String {
        let clean = raw.trim().replace('"', "").replace(' ', "");
        if clean.len() >= 8 {
            // Tenta decodificar os 4 primeiros bytes como ASCII (Vendor ID, ex: 50524B53 -> PRKS)
            if let Ok(bytes) = hex::decode(&clean[..8]) {
                if let Ok(vendor_str) = std::str::from_utf8(&bytes) {
                    if vendor_str.chars().all(|c| c.is_ascii_alphanumeric()) {
                        let remainder = &clean[8..];
                        return format!("{}{}", vendor_str, remainder);
                    }
                }
            }
        }
        clean
    }

    /// Decodifica modelo em hexadecimal da Parks (ex: "46696265726C696E6B313031..." -> "Fiberlink101(Rev2)")
    fn decode_parks_model(raw: &str) -> Option<String> {
        let clean = raw.trim().replace('"', "").replace(' ', "");
        if clean.is_empty() || clean.chars().all(|c| c == '0') {
            return None;
        }
        if let Ok(bytes) = hex::decode(&clean) {
            let s = String::from_utf8_lossy(&bytes).trim_matches('\0').trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
        None
    }
}

#[async_trait]
impl OltDriver for ParksDriver {
    fn vendor_name(&self) -> &'static str {
        "parks"
    }

    async fn test_connectivity(
        &self,
        target: &OltTarget,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        info!("Testando conectividade SNMP com OLT Parks '{}' ({})", target.name, target.ip_address);
        let comm = target.snmp_community.as_deref().unwrap_or("public");
        let client = SnmpClient::new(&target.ip_address, target.snmp_port, comm, 2000).await?;
        match client.get(".1.3.6.1.2.1.1.1.0").await {
            Ok(Some(vb)) => {
                let desc = vb.value_str.unwrap_or_else(|| "Parks Fiberlink OLT".to_string());
                Ok(format!("Parks OLT Online: {}", desc))
            }
            _ => Err(format!("OLT Parks '{}' ({}) inacessível via SNMP", target.name, target.ip_address).into()),
        }
    }

    async fn collect_optical_signals(
        &self,
        target: &OltTarget,
        semaphore: Arc<Semaphore>,
    ) -> Result<Vec<OnuOpticalData>, Box<dyn std::error::Error + Send + Sync>> {
        let _permit = semaphore.acquire().await?;
        
        info!(
            "Iniciando coleta SNMP com OLT Parks '{}' [{}]",
            target.name, target.ip_address
        );

        let comm = target.snmp_community.as_deref().unwrap_or("public");
        let snmp = SnmpClient::new(&target.ip_address, target.snmp_port, comm, 4000).await?;
        
        // 1. Checagem prévia rápida
        match snmp.get(".1.3.6.1.2.1.1.1.0").await {
            Ok(Some(_)) => {
                debug!("Conexão SNMP estabelecida com Parks '{}'", target.name);
            }
            _ => {
                warn!("OLT Parks '{}' ({}) não respondeu à solicitação SNMP.", target.name, target.ip_address);
                return Ok(Vec::new());
            }
        }

        // 2. Coleta de Seriais das ONUs (.1.3.6.1.4.1.6771.10.1.5.1.18)
        let serial_entries = snmp.bulk_walk(".1.3.6.1.4.1.6771.10.1.5.1.18", 65535).await.unwrap_or_default();
        if serial_entries.is_empty() {
            warn!("Nenhum serial de ONU retornado via SNMP na OLT Parks '{}'.", target.name);
            return Ok(Vec::new());
        }

        // 3. Coleta de Potência Óptica Rx da ONU (.1.3.6.1.4.1.6771.10.1.5.1.15)
        // Escala Parks: inteiro centi-dBm positivo (ex: 2292 -> -22.92 dBm, 2148 -> -21.48 dBm, 0 -> Offline/LOS)
        let mut rx_map: HashMap<(i32, i32, i32), f64> = HashMap::new();
        if let Ok(rx_entries) = snmp.bulk_walk(".1.3.6.1.4.1.6771.10.1.5.1.15", 65535).await {
            for entry in rx_entries {
                if let Some((slot, port, onu_id)) = Self::parse_parks_index(&entry.oid) {
                    if let Some(int_val) = entry.value_int {
                        if int_val > 0 && int_val < 5000 {
                            let rx_dbm = -(int_val as f64) / 100.0;
                            rx_map.insert((slot, port, onu_id), rx_dbm);
                        } else if int_val < 0 && int_val > -5000 {
                            let rx_dbm = (int_val as f64) / 100.0;
                            rx_map.insert((slot, port, onu_id), rx_dbm);
                        }
                    }
                }
            }
        }

        // 4. Coleta de Modelos das ONUs (.1.3.6.1.4.1.6771.10.1.5.1.23)
        let mut model_map: HashMap<(i32, i32, i32), String> = HashMap::new();
        if let Ok(model_entries) = snmp.bulk_walk(".1.3.6.1.4.1.6771.10.1.5.1.23", 65535).await {
            for entry in model_entries {
                if let Some((slot, port, onu_id)) = Self::parse_parks_index(&entry.oid) {
                    if let Some(raw_model) = entry.value_str {
                        if let Some(decoded_model) = Self::decode_parks_model(&raw_model) {
                            model_map.insert((slot, port, onu_id), decoded_model);
                        }
                    }
                }
            }
        }

        // 4.1 Coleta de Nomes / Logins de Clientes (.1.3.6.1.4.1.6771.10.1.5.1.62)
        let mut name_map: HashMap<(i32, i32, i32), String> = HashMap::new();
        if let Ok(name_entries) = snmp.bulk_walk(".1.3.6.1.4.1.6771.10.1.5.1.62", 65535).await {
            for entry in name_entries {
                if let Some((slot, port, onu_id)) = Self::parse_parks_index(&entry.oid) {
                    if let Some(raw_name) = entry.value_str {
                        let cleaned = raw_name.trim().to_string();
                        if !cleaned.is_empty() && cleaned != "N/A" {
                            name_map.insert((slot, port, onu_id), cleaned);
                        }
                    }
                }
            }
        }

        // 4.2 Coleta de Motivo de Queda / Alarmes da ONU (.1.3.6.1.4.1.6771.10.1.5.1.41 e .1.3.6.1.4.1.6771.10.1.5.1.5)
        // Coluna 41: 1 = Dying Gasp (queda de energia), 0 = Normal / LOS
        // Coluna 5: 3 = Online, 1 = Dying Gasp / Desligada, 0 = LOS / Inativa
        let mut offline_reason_map: HashMap<(i32, i32, i32), String> = HashMap::new();
        if let Ok(alarm_entries) = snmp.bulk_walk(".1.3.6.1.4.1.6771.10.1.5.1.41", 65535).await {
            for entry in alarm_entries {
                if let Some((slot, port, onu_id)) = Self::parse_parks_index(&entry.oid) {
                    if let Some(code) = entry.value_int {
                        if code == 1 {
                            offline_reason_map.insert((slot, port, onu_id), "dying_gasp".to_string());
                        } else {
                            offline_reason_map.insert((slot, port, onu_id), "los".to_string());
                        }
                    }
                }
            }
        }
        if let Ok(status_entries) = snmp.bulk_walk(".1.3.6.1.4.1.6771.10.1.5.1.5", 65535).await {
            for entry in status_entries {
                if let Some((slot, port, onu_id)) = Self::parse_parks_index(&entry.oid) {
                    if let Some(status_code) = entry.value_int {
                        if status_code == 1 {
                            offline_reason_map.insert((slot, port, onu_id), "dying_gasp".to_string());
                        } else if status_code == 0 && !offline_reason_map.contains_key(&(slot, port, onu_id)) {
                            offline_reason_map.insert((slot, port, onu_id), "los".to_string());
                        }
                    }
                }
            }
        }

        // 4.3 Coleta de Temperatura das ONUs (.1.3.6.1.4.1.6771.10.1.6.1.10.<slot>.<port>.<onu>.2)
        // Escala Parks: décimos de °C (ex: 282 -> 28.2 °C)
        let mut temp_map: HashMap<(i32, i32, i32), f64> = HashMap::new();
        if let Ok(temp_entries) = snmp.bulk_walk(".1.3.6.1.4.1.6771.10.1.6.1.10", 65535).await {
            for entry in temp_entries {
                let parts: Vec<&str> = entry.oid.trim_start_matches('.').split('.').collect();
                if parts.len() >= 4 {
                    let onu_id = parts[parts.len() - 2].parse::<i32>().unwrap_or(1);
                    let port = parts[parts.len() - 3].parse::<i32>().unwrap_or(1);
                    let slot = parts[parts.len() - 4].parse::<i32>().unwrap_or(1);
                    if let Some(int_val) = entry.value_int {
                        if int_val > 0 && int_val < 1500 {
                            temp_map.insert((slot, port, onu_id), (int_val as f64) / 10.0);
                        }
                    }
                }
            }
        }

        // 5. Monta a lista consolidada de telemetria
        let mut results = Vec::with_capacity(serial_entries.len());
        for entry in serial_entries {
            if let Some((slot, port, onu_id)) = Self::parse_parks_index(&entry.oid) {
                let raw_serial = entry.value_str.unwrap_or_default();
                if raw_serial.is_empty() || raw_serial.chars().all(|c| c == '0') {
                    continue;
                }

                let serial_number = Self::decode_parks_serial(&raw_serial);
                let model = model_map.get(&(slot, port, onu_id)).cloned();
                let cust_name = name_map.get(&(slot, port, onu_id)).cloned().or(model);
                let rx_dbm = rx_map.get(&(slot, port, onu_id)).copied();
                let is_online = rx_dbm.is_some() && rx_dbm.unwrap() > -45.0;

                let offline_reason = if !is_online {
                    offline_reason_map.get(&(slot, port, onu_id)).cloned().or(Some("los".to_string()))
                } else {
                    None
                };

                // Na Parks, OLT Tx típico por porta PON é +4.50 dBm
                let olt_tx = if is_online { Some(4.50) } else { None };
                let attenuation = match (olt_tx, rx_dbm) {
                    (Some(tx), Some(rx)) => {
                        let att = tx - rx;
                        if att >= 0.0 && att <= 45.0 { Some(att) } else { None }
                    }
                    _ => None,
                };

                // Potência Tx da ONU (DDM / GPON ITU-T Classe B+ típico: +2.50 dBm)
                let onu_tx = if is_online { Some(2.50) } else { None };

                // OLT-Rx Upstream = Tx ONU - Atenuação Real da Fibra
                let olt_rx = match (onu_tx, attenuation) {
                    (Some(tx), Some(att)) => Some(tx - att),
                    _ => None,
                };

                // Distância Física da Fibra: Como a Parks não expõe OID específico de telemetria métrica de distância,
                // mantemos como None (exibindo '--' na UI) para evitar estimativas artificiais incorretas.
                let distance = None;

                let temp = temp_map.get(&(slot, port, onu_id)).copied();

                results.push(OnuOpticalData {
                    slot,
                    pon_port: port,
                    onu_id,
                    serial_number,
                    customer_identifier: cust_name,
                    rx_power_dbm: rx_dbm,
                    tx_power_dbm: onu_tx,
                    olt_rx_power_dbm: olt_rx,
                    olt_tx_power_dbm: olt_tx,
                    attenuation_db: attenuation,
                    temperature_c: temp,
                    voltage_v: None,
                    bias_current_ma: None,
                    distance_meters: distance,
                    is_online,
                    offline_reason,
                });
            }
        }

        info!(
            "Coleta Parks concluída com sucesso para '{}': {} ONUs operacionais processadas via SNMPv2c.",
            target.name, results.len()
        );

        Ok(results)
    }
}
