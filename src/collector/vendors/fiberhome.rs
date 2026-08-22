use async_trait::async_trait;
use log::{debug, info, warn};
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::collector::driver::{OltDriver, OltTarget, OnuOpticalData};
use crate::collector::snmp::SnmpClient;

/// Driver FiberHome de Alta Precisão para OLTs AN5516 / AN5116 (100% SNMPv2c)
pub struct FiberHomeDriver;

impl FiberHomeDriver {
    pub fn new() -> Self {
        Self
    }

    /// Decodificação precisa de Slot, Porta PON e ONU ID a partir do índice de OID da FiberHome AN5516
    /// Exemplo: 369623296 (0x16080100) -> Slot 11, Porta 1, ONU 1
    /// Formato binário oficial AN5516:
    /// - (raw >> 24) / 2 -> Slot (ex: 0x16 = 22 -> 22 / 2 = Slot 11)
    /// - ((raw >> 16) & 0xFF) / 8 -> Porta PON (ex: 0x08 = 8 -> 8 / 8 = Port 1, 0x10 = 16 -> 16 / 8 = Port 2)
    /// - (raw >> 8) & 0xFF -> ONU ID (ex: 0x01 = 1 -> ONU 1)
    fn parse_fiberhome_index(oid: &str, default_idx: usize) -> (i32, i32, i32) {
        let parts: Vec<&str> = oid.trim_start_matches('.').split('.').collect();
        if let Some(last) = parts.last() {
            if let Ok(raw) = last.parse::<u32>() {
                if raw >= 0x01000000 {
                    let slot = ((raw >> 24) / 2) as i32;
                    let port = (((raw >> 16) & 0xFF) / 8).max(1) as i32;
                    let onu = ((raw >> 8) & 0xFF).max(1) as i32;
                    return (slot.max(1), port, onu);
                }
            }
        }

        if parts.len() >= 3 {
            let onu_id = parts
                .last()
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(1);
            let port = parts
                .get(parts.len() - 2)
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(1);
            let slot = parts
                .get(parts.len() - 3)
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(1);
            return (slot, port, onu_id);
        }

        let total = 16 * 64;
        let slot = ((default_idx / total) + 1) as i32;
        let port = (((default_idx % total) / 64) + 1) as i32;
        let onu = ((default_idx % 64) + 1) as i32;
        (slot, port, onu)
    }

    /// Extrai chave de sufixo normalizada a partir do OID
    fn get_oid_suffix(oid: &str) -> String {
        let parts: Vec<&str> = oid.trim_start_matches('.').split('.').collect();
        if let Some(last) = parts.last() {
            last.to_string()
        } else {
            oid.to_string()
        }
    }
}

#[async_trait]
impl OltDriver for FiberHomeDriver {
    fn vendor_name(&self) -> &'static str {
        "fiberhome"
    }

    async fn test_connectivity(
        &self,
        target: &OltTarget,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Testando conectividade SNMP com OLT FiberHome '{}' ({})",
            target.name, target.ip_address
        );
        let comm = target.snmp_community.as_deref().unwrap_or("public");
        let client = SnmpClient::new(&target.ip_address, target.snmp_port, comm, 2000).await?;
        match client.get(".1.3.6.1.2.1.1.1.0").await {
            Ok(Some(vb)) => {
                let desc = vb
                    .value_str
                    .unwrap_or_else(|| "FiberHome AN5516".to_string());
                Ok(format!("FiberHome OLT Online: {}", desc))
            }
            _ => Err(format!(
                "OLT FiberHome '{}' ({}) inacessível via SNMP",
                target.name, target.ip_address
            )
            .into()),
        }
    }

    async fn collect_optical_signals(
        &self,
        target: &OltTarget,
        semaphore: Arc<Semaphore>,
    ) -> Result<Vec<OnuOpticalData>, Box<dyn std::error::Error + Send + Sync>> {
        let _permit = semaphore.acquire().await?;

        info!(
            "Iniciando coleta SNMP de Alta Velocidade com OLT FiberHome '{}' [{}]",
            target.name, target.ip_address
        );

        let mut results = Vec::new();
        let comm = target.snmp_community.as_deref().unwrap_or("public");
        let snmp = SnmpClient::new(&target.ip_address, target.snmp_port, comm, 4000).await?;

        // 1. Checagem prévia rápida
        match snmp.get(".1.3.6.1.2.1.1.1.0").await {
            Ok(Some(_)) => {
                debug!("Conexão SNMP estabelecida com FiberHome '{}'", target.name);
            }
            _ => {
                warn!(
                    "OLT FiberHome '{}' ({}) não respondeu à solicitação SNMP.",
                    target.name, target.ip_address
                );
                return Err(format!(
                    "OLT FiberHome '{}' ({}) inacessível via SNMP (Timeout)",
                    target.name, target.ip_address
                )
                .into());
            }
        }

        // 2. Mapeamento de Potências Ópticas Reais das ONUs
        // Tabela Oficial de Diagnóstico Óptico FiberHome AN5516 / AN5116 (.1.3.6.1.4.1.5875.800.3.9.3.3.1):
        // .6  = Potência Óptica Rx da ONU (Downstream em centésimos de dBm: -2284 -> -22.84 dBm)
        // .7  = Potência Óptica Tx da ONU (Upstream em centésimos de dBm: 242 -> +2.42 dBm)
        // .8  = Temperatura do Transceiver (décimos de °C: 324 -> 32.4 °C)
        // .9  = Tensão / Voltagem do Transceiver (em mV: 1501 -> 3.3V / 1.5V)
        // .10 = Corrente de Bias do Laser (em uA: 5166 -> 5.16 mA)
        let mut rx_map = std::collections::HashMap::new();
        let mut tx_map = std::collections::HashMap::new();
        let mut temp_map = std::collections::HashMap::new();
        let mut volt_map = std::collections::HashMap::new();
        let mut bias_map = std::collections::HashMap::new();

        let diag_rx_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.5875.800.3.9.3.3.1.6", 65535)
            .await
            .unwrap_or_default();
        for vb in diag_rx_walk {
            let key = Self::get_oid_suffix(&vb.oid);
            if let Some(raw) = vb.value_int {
                if raw != 0 && raw != 2147483647 && raw != -1 && raw != -8000 {
                    let dbm = (raw as f64) / 100.0;
                    if dbm > -60.0 && dbm < 10.0 {
                        rx_map.insert(key, dbm);
                    }
                }
            }
        }

        let diag_tx_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.5875.800.3.9.3.3.1.7", 65535)
            .await
            .unwrap_or_default();
        for vb in diag_tx_walk {
            let key = Self::get_oid_suffix(&vb.oid);
            if let Some(raw) = vb.value_int {
                if raw != 0 && raw != 2147483647 && raw != -1 {
                    let dbm = (raw as f64) / 100.0;
                    if dbm > -10.0 && dbm < 15.0 {
                        tx_map.insert(key, dbm);
                    }
                }
            }
        }

        let diag_temp_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.5875.800.3.9.3.3.1.8", 65535)
            .await
            .unwrap_or_default();
        for vb in diag_temp_walk {
            let key = Self::get_oid_suffix(&vb.oid);
            if let Some(raw) = vb.value_int {
                if raw > 0 && raw < 1500 {
                    temp_map.insert(key, (raw as f64) / 10.0);
                }
            }
        }

        let diag_volt_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.5875.800.3.9.3.3.1.9", 65535)
            .await
            .unwrap_or_default();
        for vb in diag_volt_walk {
            let key = Self::get_oid_suffix(&vb.oid);
            if let Some(raw) = vb.value_int {
                if raw > 0 && raw < 10000 {
                    volt_map.insert(key, (raw as f64) / 1000.0);
                }
            }
        }

        let diag_bias_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.5875.800.3.9.3.3.1.10", 65535)
            .await
            .unwrap_or_default();
        for vb in diag_bias_walk {
            let key = Self::get_oid_suffix(&vb.oid);
            if let Some(raw) = vb.value_int {
                if raw > 0 && raw < 200000 {
                    bias_map.insert(key, (raw as f64) / 1000.0);
                }
            }
        }

        // 3. Mapeamento de Distância Física em Metros (.1.3.6.1.4.1.5875.800.3.10.1.1.5)
        let mut dist_map = std::collections::HashMap::new();
        let dist_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.5875.800.3.10.1.1.5", 65535)
            .await
            .unwrap_or_default();
        for vb in dist_walk {
            let key = Self::get_oid_suffix(&vb.oid);
            if let Some(raw) = vb.value_int {
                if raw > 0 && raw < 80000 {
                    dist_map.insert(key, raw as i32);
                }
            }
        }

        // 4. Mapeamento de Nomes / Descrições de Clientes (.1.3.6.1.4.1.5875.800.3.10.1.1.21 e .20)
        let mut name_map = std::collections::HashMap::new();
        let name_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.5875.800.3.10.1.1.21", 65535)
            .await
            .unwrap_or_default();
        for vb in name_walk {
            let key = Self::get_oid_suffix(&vb.oid);
            if let Some(ref name) = vb.value_str {
                let cleaned = name.trim();
                if !cleaned.is_empty() && cleaned != "N/A" {
                    name_map.insert(key, cleaned.to_string());
                }
            }
        }

        // 5. Mapeamento de Status de Operação (.1.3.6.1.4.1.5875.800.3.10.1.1.25)
        // 1: Online / Authenticated, 0: Offline
        let mut status_map = std::collections::HashMap::new();
        let status_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.5875.800.3.10.1.1.25", 65535)
            .await
            .unwrap_or_default();
        for vb in status_walk {
            let key = Self::get_oid_suffix(&vb.oid);
            if let Some(raw) = vb.value_int {
                status_map.insert(key, raw == 1);
            }
        }

        // 6. Mapeamento de Potência Tx dos SFPs PON da OLT (.1.3.6.1.4.1.5875.800.3.9.3.4.1.8)
        // Ex: 517 -> +5.17 dBm por porta PON (Index: (slot * 2) << 24 | (port * 8) << 16)
        let mut olt_pon_tx_map = std::collections::HashMap::new();
        let olt_tx_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.5875.800.3.9.3.4.1.8", 65535)
            .await
            .unwrap_or_default();
        for vb in olt_tx_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if let Some(last) = parts.last() {
                if let Ok(raw) = last.parse::<u32>() {
                    let slot = ((raw >> 24) / 2) as i32;
                    let port = (((raw >> 16) & 0xFF) / 8).max(1) as i32;
                    if let Some(raw_tx) = vb.value_int {
                        if raw_tx > 0 && raw_tx < 1500 {
                            olt_pon_tx_map.insert((slot, port), (raw_tx as f64) / 100.0);
                        }
                    }
                }
            }
        }

        // 7. Seriais das ONUs FiberHome (.1.3.6.1.4.1.5875.800.3.10.1.1.10)
        let onu_table_primary = snmp
            .bulk_walk(".1.3.6.1.4.1.5875.800.3.10.1.1.10", 65535)
            .await
            .unwrap_or_default();
        let onu_table = if !onu_table_primary.is_empty() {
            onu_table_primary
        } else {
            snmp.bulk_walk(".1.3.6.1.4.1.5875.800.3.10.1.1.3", 65535)
                .await
                .unwrap_or_default()
        };

        if !onu_table.is_empty() {
            for (idx, vb) in onu_table.iter().enumerate() {
                let key = Self::get_oid_suffix(&vb.oid);
                let (slot, pon_port, onu_id) = Self::parse_fiberhome_index(&vb.oid, idx);

                let raw_serial = if let Some(ref s) = vb.value_str {
                    s.trim().to_string()
                } else if !vb.value_raw.is_empty() {
                    String::from_utf8_lossy(&vb.value_raw).trim().to_string()
                } else {
                    format!("FHTT{:08X}", idx + 0x1000)
                };

                let serial = if raw_serial.len() >= 8 {
                    raw_serial
                } else {
                    format!("FHTT{:08X}", idx + 0x1000)
                };

                let rx_dbm = rx_map.get(&key).copied();
                let tx_dbm = tx_map.get(&key).copied();
                let temp_c = temp_map.get(&key).copied();
                let volt_v = volt_map.get(&key).copied();
                let bias_ma = bias_map.get(&key).copied();
                let distance_m = dist_map.get(&key).copied();
                let cust_name = name_map.get(&key).cloned();

                let is_online = status_map
                    .get(&key)
                    .copied()
                    .unwrap_or(rx_dbm.is_some() && rx_dbm.unwrap() > -45.0);

                let olt_tx = olt_pon_tx_map
                    .get(&(slot, pon_port))
                    .copied()
                    .unwrap_or(5.00);

                let attenuation_db = if is_online {
                    if let Some(rx) = rx_dbm {
                        let att = olt_tx - rx;
                        if att >= 0.0 && att <= 45.0 {
                            Some(att)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                // OLT Rx Power (Upstream recebido no chassi):
                // Se a ONU transmite tx_dbm e a fibra possui atenuação 'att', o sinal que chega na OLT é (tx_dbm - att)
                let olt_rx_dbm = if is_online {
                    if let (Some(tx), Some(att)) = (tx_dbm, attenuation_db) {
                        let calc_rx = tx - att;
                        if calc_rx > -45.0 && calc_rx < 0.0 {
                            Some((calc_rx * 100.0).round() / 100.0)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                results.push(OnuOpticalData {
                    slot,
                    pon_port,
                    onu_id,
                    serial_number: serial,
                    customer_identifier: cust_name,
                    rx_power_dbm: rx_dbm,
                    tx_power_dbm: tx_dbm,
                    olt_rx_power_dbm: olt_rx_dbm,
                    olt_tx_power_dbm: Some(olt_tx),
                    attenuation_db,
                    temperature_c: temp_c,
                    voltage_v: volt_v,
                    bias_current_ma: bias_ma,
                    distance_meters: distance_m,
                    is_online,
                    offline_reason: if !is_online {
                        Some("los".to_string())
                    } else {
                        None
                    },
                });
            }
        } else {
            warn!(
                "FiberHome OLT '{}' ({}) não retornou registros na MIB de ONUs.",
                target.name, target.ip_address
            );
            return Err(format!(
                "OLT '{}' ({}) inacessível ou sem resposta na MIB de ONUs",
                target.name, target.ip_address
            )
            .into());
        }

        // Deduplica entradas por Serial único ou (slot, pon_port, onu_id)
        let mut unique_map: std::collections::HashMap<String, OnuOpticalData> =
            std::collections::HashMap::new();
        for item in results {
            let key = if !item.serial_number.is_empty() {
                item.serial_number.clone()
            } else {
                format!("{}.{}.{}", item.slot, item.pon_port, item.onu_id)
            };
            if let Some(existing) = unique_map.get_mut(&key) {
                if !existing.is_online && item.is_online {
                    *existing = item;
                }
            } else {
                unique_map.insert(key, item);
            }
        }

        let final_results: Vec<OnuOpticalData> = unique_map.into_values().collect();
        info!(
            "Coleta SNMP da OLT FiberHome '{}' finalizada com {} ONUs lidas do equipamento.",
            target.name,
            final_results.len()
        );
        Ok(final_results)
    }
}
