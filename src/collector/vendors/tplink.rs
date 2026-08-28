use async_trait::async_trait;
use log::info;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::Duration;

use crate::collector::driver::{OltDriver, OltTarget, OnuOpticalData};
use crate::collector::snmp::SnmpClient;

/// Driver TP-Link DeltaStream GPON OLT 100% SNMPv2c de Alta Performance
/// Atende às famílias DS-P7001 (01/04/08/16), DS-P8000 e compatíveis
pub struct TpLinkDriver;

impl TpLinkDriver {
    pub fn new() -> Self {
        Self
    }

    /// Extrai a tupla de indexação {slot, pon_port, onu_id} a partir do sufixo do OID
    fn parse_onu_index(oid: &str) -> Option<(i32, i32, i32)> {
        let parts: Vec<&str> = oid.split('.').collect();
        if parts.len() >= 3 {
            let onu_id = parts[parts.len() - 1].parse::<i32>().ok()?;
            let port = parts[parts.len() - 2].parse::<i32>().ok()?;
            let slot = parts[parts.len() - 3].parse::<i32>().ok()?;
            Some((slot, port, onu_id))
        } else {
            None
        }
    }

    /// Converte DisplayString de potência óptica (ex: "-24.50", "-24.50 dBm", "N/A", "--") para f64
    fn parse_power_string(s: &str) -> Option<f64> {
        let trimmed = s
            .trim()
            .trim_end_matches("dBm")
            .trim_end_matches("dB")
            .trim();
        if trimmed.is_empty()
            || trimmed == "--"
            || trimmed == "N/A"
            || trimmed == "null"
            || trimmed == "0"
            || trimmed == "0.0"
            || trimmed == "0.00"
        {
            return None;
        }
        if let Ok(val) = trimmed.parse::<f64>() {
            if val > -50.0 && val < 20.0 {
                return Some(val);
            }
        }
        None
    }

    /// Converte DisplayString de temperatura (ex: "45.2", "45 C", "45.2 C") para f64
    fn parse_temp_string(s: &str) -> Option<f64> {
        let trimmed = s
            .trim()
            .trim_end_matches('C')
            .trim_end_matches('c')
            .trim_end_matches("°C")
            .trim();
        if trimmed.is_empty() || trimmed == "--" || trimmed == "N/A" {
            return None;
        }
        if let Ok(val) = trimmed.parse::<f64>() {
            if val > -40.0 && val < 120.0 {
                return Some(val);
            }
        }
        None
    }

    /// Converte DisplayString de corrente de bias (mA) para f64
    fn parse_bias_string(s: &str) -> Option<f64> {
        let trimmed = s
            .trim()
            .trim_end_matches("mA")
            .trim_end_matches("ma")
            .trim();
        if trimmed.is_empty() || trimmed == "--" || trimmed == "N/A" {
            return None;
        }
        if let Ok(val) = trimmed.parse::<f64>() {
            if val >= 0.0 && val < 200.0 {
                return Some(val);
            }
        }
        None
    }
}

#[async_trait]
impl OltDriver for TpLinkDriver {
    fn vendor_name(&self) -> &'static str {
        "tplink"
    }

    async fn test_connectivity(
        &self,
        target: &OltTarget,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let community = target.snmp_community.as_deref().unwrap_or("public");
        let client = SnmpClient::new(&target.ip_address, target.snmp_port, community, 3000).await?;

        let sys_descr = client
            .get(".1.3.6.1.2.1.1.1.0")
            .await?
            .and_then(|vb| vb.value_str)
            .unwrap_or_else(|| "TP-Link DeltaStream OLT".to_string());

        let hostname = client
            .get(".1.3.6.1.2.1.1.5.0")
            .await
            .ok()
            .flatten()
            .and_then(|vb| vb.value_str)
            .unwrap_or_default();

        Ok(format!(
            "TP-Link SNMPv2c Online | Host: {} | {}",
            hostname, sys_descr
        ))
    }

    async fn collect_optical_signals(
        &self,
        target: &OltTarget,
        semaphore: Arc<Semaphore>,
    ) -> Result<Vec<OnuOpticalData>, Box<dyn std::error::Error + Send + Sync>> {
        let _permit = semaphore.acquire().await?;
        let community = target.snmp_community.as_deref().unwrap_or("public");
        let client = SnmpClient::new(&target.ip_address, target.snmp_port, community, 3500).await?;

        info!(
            "TP-Link '{}' [{}]: Iniciando varredura 100% SNMPv2c (DeltaStream GPON OLT)...",
            target.name, target.ip_address
        );

        // ── 1. Mapeamento de Seriais das ONUs (omSerialNumber: .1.3.6.1.4.1.11863.6.100.1.7.2.1.6) ──
        let mut serial_map: HashMap<(i32, i32, i32), String> = HashMap::new();
        let serial_vbs = client
            .walk(
                ".1.3.6.1.4.1.11863.6.100.1.7.2.1.6",
                1000,
                Duration::from_millis(5),
            )
            .await
            .unwrap_or_default();

        for vb in serial_vbs {
            if let Some(idx) = Self::parse_onu_index(&vb.oid) {
                if let Some(mut sn) = vb.value_str {
                    sn = sn.trim().to_string();
                    if !sn.is_empty()
                        && sn != "000000000000"
                        && !sn.chars().all(|c| c == '\0' || c == ' ')
                    {
                        serial_map.insert(idx, sn);
                    }
                }
            }
        }

        info!(
            "TP-Link '{}': {} ONUs identificadas com número de série.",
            target.name,
            serial_map.len()
        );

        if serial_map.is_empty() {
            return Ok(Vec::new());
        }

        // ── 2. Descrição / Nome do Cliente (omOnuDescription: .1.3.6.1.4.1.11863.6.100.1.7.2.1.5) ──
        let mut descr_map: HashMap<(i32, i32, i32), String> = HashMap::new();
        if let Ok(descr_vbs) = client
            .walk(
                ".1.3.6.1.4.1.11863.6.100.1.7.2.1.5",
                1000,
                Duration::from_millis(5),
            )
            .await
        {
            for vb in descr_vbs {
                if let Some(idx) = Self::parse_onu_index(&vb.oid) {
                    if let Some(d) = vb.value_str {
                        let trimmed = d.trim().to_string();
                        if !trimmed.is_empty() {
                            descr_map.insert(idx, trimmed);
                        }
                    }
                }
            }
        }

        // ── 3. Status Online / Offline (omOnlineStatus: .1.3.6.1.4.1.11863.6.100.1.7.2.1.11) ──
        let mut status_map: HashMap<(i32, i32, i32), bool> = HashMap::new();
        if let Ok(status_vbs) = client
            .walk(
                ".1.3.6.1.4.1.11863.6.100.1.7.2.1.11",
                1000,
                Duration::from_millis(5),
            )
            .await
        {
            for vb in status_vbs {
                if let Some(idx) = Self::parse_onu_index(&vb.oid) {
                    // 1 = online, 0 = offline
                    let is_on = vb.value_int.map(|v| v == 1).unwrap_or_else(|| {
                        vb.value_str
                            .as_deref()
                            .map(|s| s.trim() == "1" || s.to_lowercase().contains("online"))
                            .unwrap_or(false)
                    });
                    status_map.insert(idx, is_on);
                }
            }
        }

        // ── 4. Potência Óptica Rx da ONU (omReceivedOpticalPower: .1.3.6.1.4.1.11863.6.100.1.7.2.1.26) ──
        let mut rx_map: HashMap<(i32, i32, i32), f64> = HashMap::new();
        if let Ok(rx_vbs) = client
            .walk(
                ".1.3.6.1.4.1.11863.6.100.1.7.2.1.26",
                1000,
                Duration::from_millis(5),
            )
            .await
        {
            for vb in rx_vbs {
                if let Some(idx) = Self::parse_onu_index(&vb.oid) {
                    if let Some(rx_str) = vb.value_str {
                        if let Some(val) = Self::parse_power_string(&rx_str) {
                            rx_map.insert(idx, val);
                        }
                    }
                }
            }
        }

        // ── 5. Potência Óptica Tx da ONU (omTransmittedOpticalPower: .1.3.6.1.4.1.11863.6.100.1.7.2.1.27) ──
        let mut tx_map: HashMap<(i32, i32, i32), f64> = HashMap::new();
        if let Ok(tx_vbs) = client
            .walk(
                ".1.3.6.1.4.1.11863.6.100.1.7.2.1.27",
                1000,
                Duration::from_millis(5),
            )
            .await
        {
            for vb in tx_vbs {
                if let Some(idx) = Self::parse_onu_index(&vb.oid) {
                    if let Some(tx_str) = vb.value_str {
                        if let Some(val) = Self::parse_power_string(&tx_str) {
                            tx_map.insert(idx, val);
                        }
                    }
                }
            }
        }

        // ── 6. Potência Óptica Rx na OLT (omOltReceivedOpticalPower: .1.3.6.1.4.1.11863.6.100.1.7.2.1.28) ──
        let mut olt_rx_map: HashMap<(i32, i32, i32), f64> = HashMap::new();
        if let Ok(olt_rx_vbs) = client
            .walk(
                ".1.3.6.1.4.1.11863.6.100.1.7.2.1.28",
                1000,
                Duration::from_millis(5),
            )
            .await
        {
            for vb in olt_rx_vbs {
                if let Some(idx) = Self::parse_onu_index(&vb.oid) {
                    if let Some(olt_rx_str) = vb.value_str {
                        if let Some(val) = Self::parse_power_string(&olt_rx_str) {
                            olt_rx_map.insert(idx, val);
                        }
                    }
                }
            }
        }

        // ── 7. Distância Física (omDistance: .1.3.6.1.4.1.11863.6.100.1.7.2.1.18) ──
        let mut distance_map: HashMap<(i32, i32, i32), i32> = HashMap::new();
        if let Ok(dist_vbs) = client
            .walk(
                ".1.3.6.1.4.1.11863.6.100.1.7.2.1.18",
                1000,
                Duration::from_millis(5),
            )
            .await
        {
            for vb in dist_vbs {
                if let Some(idx) = Self::parse_onu_index(&vb.oid) {
                    if let Some(d) = vb.value_int {
                        if d > 0 && d < 100000 {
                            distance_map.insert(idx, d as i32);
                        }
                    }
                }
            }
        }

        // ── 8. Diagnósticos DDM (Temperatura, Voltagem, Corrente de Bias) ──
        let mut temp_map: HashMap<(i32, i32, i32), f64> = HashMap::new();
        if let Ok(temp_vbs) = client
            .walk(
                ".1.3.6.1.4.1.11863.6.100.1.7.2.1.31",
                1000,
                Duration::from_millis(5),
            )
            .await
        {
            for vb in temp_vbs {
                if let Some(idx) = Self::parse_onu_index(&vb.oid) {
                    if let Some(t_str) = vb.value_str {
                        if let Some(val) = Self::parse_temp_string(&t_str) {
                            temp_map.insert(idx, val);
                        }
                    }
                }
            }
        }

        let mut volt_map: HashMap<(i32, i32, i32), f64> = HashMap::new();
        if let Ok(volt_vbs) = client
            .walk(
                ".1.3.6.1.4.1.11863.6.100.1.7.2.1.30",
                1000,
                Duration::from_millis(5),
            )
            .await
        {
            for vb in volt_vbs {
                if let Some(idx) = Self::parse_onu_index(&vb.oid) {
                    // Valor em mV (ex: 3300 mV -> 3.3V)
                    if let Some(mv) = vb.value_int {
                        if mv > 0 && mv < 10000 {
                            volt_map.insert(idx, mv as f64 / 1000.0);
                        }
                    }
                }
            }
        }

        let mut bias_map: HashMap<(i32, i32, i32), f64> = HashMap::new();
        if let Ok(bias_vbs) = client
            .walk(
                ".1.3.6.1.4.1.11863.6.100.1.7.2.1.29",
                1000,
                Duration::from_millis(5),
            )
            .await
        {
            for vb in bias_vbs {
                if let Some(idx) = Self::parse_onu_index(&vb.oid) {
                    if let Some(b_str) = vb.value_str {
                        if let Some(val) = Self::parse_bias_string(&b_str) {
                            bias_map.insert(idx, val);
                        }
                    }
                }
            }
        }

        // ── 9. Causa da Última Desconexão (omOnuLastDownCauses: .1.3.6.1.4.1.11863.6.100.1.7.2.1.42) ──
        let mut down_reason_map: HashMap<(i32, i32, i32), String> = HashMap::new();
        if let Ok(reason_vbs) = client
            .walk(
                ".1.3.6.1.4.1.11863.6.100.1.7.2.1.42",
                1000,
                Duration::from_millis(5),
            )
            .await
        {
            for vb in reason_vbs {
                if let Some(idx) = Self::parse_onu_index(&vb.oid) {
                    if let Some(r) = vb.value_str {
                        let trimmed = r.trim().to_string();
                        if !trimmed.is_empty() && trimmed != "--" {
                            down_reason_map.insert(idx, trimmed);
                        }
                    }
                }
            }
        }

        // ── 10. Consolidação dos dados coletados ──
        let mut results = Vec::with_capacity(serial_map.len());

        for (idx @ (slot, pon_port, onu_id), serial_number) in serial_map {
            let is_online = status_map.get(&idx).copied().unwrap_or(false);
            let rx = rx_map.get(&idx).copied();
            let tx = tx_map.get(&idx).copied();
            let olt_rx = olt_rx_map.get(&idx).copied();
            let distance = distance_map.get(&idx).copied();
            let temp = temp_map.get(&idx).copied();
            let volt = volt_map.get(&idx).copied();
            let bias = bias_map.get(&idx).copied();
            let customer_identifier = descr_map.get(&idx).cloned();
            let offline_reason = down_reason_map.get(&idx).cloned();

            // Cálculo da atenuação óptica total (se OLT Tx padrão ~ +2.5 dBm ou valor de porta disponível)
            let attenuation_db = match (rx, tx) {
                (Some(rx_val), Some(tx_val)) if tx_val > rx_val => Some(tx_val - rx_val),
                _ => None,
            };

            results.push(OnuOpticalData {
                slot,
                pon_port,
                onu_id,
                serial_number,
                customer_identifier,
                rx_power_dbm: rx,
                tx_power_dbm: tx,
                olt_rx_power_dbm: olt_rx,
                olt_tx_power_dbm: None,
                attenuation_db,
                temperature_c: temp,
                voltage_v: volt,
                bias_current_ma: bias,
                distance_meters: distance,
                is_online,
                offline_reason,
            });
        }

        info!(
            "TP-Link '{}': Coleta finalizada com sucesso. {} ONUs processadas.",
            target.name,
            results.len()
        );

        Ok(results)
    }
}
