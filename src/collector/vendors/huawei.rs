use async_trait::async_trait;
use log::{debug, info, warn};
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::collector::driver::{OltDriver, OltTarget, OnuOpticalData};
use crate::collector::snmp::SnmpClient;

/// Driver Huawei Nativo de Alta Fidelidade e Máxima Performance
/// - 100% Rust Puro (Zero dependência de interpretadores externos como Python)
/// - SNMPv2c assíncrono com GetBulk de alta velocidade (Seriais, Nomes, Rx, Tx, OLT Rx, Temp, Voltagem, Distâncias Reais, Quedas)
pub struct HuaweiDriver;

impl HuaweiDriver {
    pub fn new() -> Self {
        Self
    }

    /// Decodifica o ifIndex da Huawei (ex: 4194328576 = 0xFA006000 -> GPON 0/3/0)
    /// Bitfield padrão Huawei VRP GPON:
    /// - Slot: (if_index >> 13) & 0x3F
    /// - PON Port: (if_index >> 8) & 0x1F
    fn parse_huawei_index(oid: &str, default_idx: usize) -> (i32, i32, i32) {
        let parts: Vec<&str> = oid.trim_start_matches('.').split('.').collect();
        if parts.len() >= 2 {
            let onu_id = parts
                .last()
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(1);
            let if_index = parts
                .get(parts.len() - 2)
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);

            if if_index > 0 {
                let slot = ((if_index >> 13) & 0x3F) as i32;
                let port = ((if_index >> 8) & 0x1F) as i32;
                return (slot, port, onu_id);
            }
        }
        let total = 16 * 128;
        let slot = ((default_idx / total) + 1) as i32;
        let port = (((default_idx % total) / 128) + 1) as i32;
        let onu = ((default_idx % 128) + 1) as i32;
        (slot, port, onu)
    }
}

#[async_trait]
impl OltDriver for HuaweiDriver {
    fn vendor_name(&self) -> &'static str {
        "huawei"
    }

    async fn test_connectivity(
        &self,
        target: &OltTarget,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Testando conectividade SNMP com OLT Huawei '{}' ({})",
            target.name, target.ip_address
        );
        let comm = target.snmp_community.as_deref().unwrap_or("public");
        let client = SnmpClient::new(&target.ip_address, target.snmp_port, comm, 2500).await?;
        match client.get(".1.3.6.1.2.1.1.1.0").await {
            Ok(Some(vb)) => {
                let desc = vb
                    .value_str
                    .unwrap_or_else(|| "Huawei SmartAX MA5800/MA5600 Series".to_string());
                Ok(format!("Huawei OLT Online: {}", desc))
            }
            _ => Err(format!(
                "OLT Huawei '{}' ({}) inacessível via SNMP",
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
            "Huawei '{}' [{}]: Iniciando Coleta SNMPv2c de alta velocidade...",
            target.name, target.ip_address
        );

        let mut results = Vec::new();
        let comm = target.snmp_community.as_deref().unwrap_or("public");
        let snmp = SnmpClient::new(&target.ip_address, target.snmp_port, comm, 3500).await?;

        // 1. Checagem prévia e identificação de Modelo e Firmware
        let (_detected_model, _detected_fw) = match snmp.get(".1.3.6.1.2.1.1.1.0").await {
            Ok(Some(vb)) => {
                debug!("Conexão SNMP estabelecida com Huawei '{}'", target.name);
                let sys_desc = vb.value_str.unwrap_or_default();
                let fw = if sys_desc.contains("SmartAX") || sys_desc.contains("MA5") {
                    sys_desc
                        .split_whitespace()
                        .find(|w| w.starts_with('V') || w.starts_with('R'))
                        .map(|s| s.to_string())
                } else {
                    None
                };
                let model = if sys_desc.contains("MA5800") {
                    Some("SmartAX MA5800 Series".to_string())
                } else if sys_desc.contains("MA5608") {
                    Some("SmartAX MA5608T".to_string())
                } else if sys_desc.contains("MA5680") {
                    Some("SmartAX MA5680T".to_string())
                } else {
                    Some("Huawei GPON OLT".to_string())
                };
                (model, fw)
            }
            _ => {
                warn!(
                    "OLT Huawei '{}' ({}) não respondeu à solicitação SNMP.",
                    target.name, target.ip_address
                );
                return Err(format!(
                    "OLT Huawei '{}' ({}) inacessível via SNMP (Timeout)",
                    target.name, target.ip_address
                )
                .into());
            }
        };

        info!(
            "Huawei '{}': Coletando telemetrias ópticas via SNMP...",
            target.name
        );

        // 2. Mapeamento de Potência Tx do SFP PON da OLT (.1.3.6.1.4.1.2011.6.128.1.1.2.23.1.2)
        let mut olt_tx_map = std::collections::HashMap::new();
        let olt_tx_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.2011.6.128.1.1.2.23.1.2", 65535)
            .await
            .unwrap_or_default();
        for vb in olt_tx_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if let Some(last) = parts.last() {
                if let Ok(if_idx) = last.parse::<u32>() {
                    let raw_val = vb.value_int.unwrap_or(0);
                    if raw_val > 0 && raw_val != 2147483647 {
                        let tx_dbm = (raw_val as f64) / 100.0;
                        if tx_dbm > -10.0 && tx_dbm < 15.0 {
                            olt_tx_map.insert(if_idx, tx_dbm);
                        }
                    }
                }
            }
        }

        // 3. Mapeamento de Seriais das ONUs (.1.3.6.1.4.1.2011.6.128.1.1.2.43.1.3)
        let onu_table = snmp
            .bulk_walk(".1.3.6.1.4.1.2011.6.128.1.1.2.43.1.3", 65535)
            .await
            .unwrap_or_default();
        if onu_table.is_empty() {
            warn!(
                "Huawei OLT '{}' ({}) não retornou registros na MIB de ONUs.",
                target.name, target.ip_address
            );
            return Err(format!(
                "OLT '{}' ({}) inacessível ou sem resposta na MIB de ONUs",
                target.name, target.ip_address
            )
            .into());
        }
        info!(
            "Huawei '{}': {} ONUs localizadas na MIB SNMP. Processando tabelas ópticas...",
            target.name,
            onu_table.len()
        );

        // 4. Mapeamento de Nomes de Clientes / Descrição (.1.3.6.1.4.1.2011.6.128.1.1.2.43.1.9)
        let mut name_map = std::collections::HashMap::new();
        let name_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.2011.6.128.1.1.2.43.1.9", 65535)
            .await
            .unwrap_or_default();
        for vb in name_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                if let Some(ref name) = vb.value_str {
                    let cleaned = name.trim();
                    if !cleaned.is_empty() && cleaned != "N/A" {
                        name_map.insert(key, cleaned.to_string());
                    }
                }
            }
        }

        // 5. Mapeamento de Modelo de Equipamento (.1.3.6.1.4.1.2011.6.128.1.1.2.45.1.4)
        let mut model_map = std::collections::HashMap::new();
        let model_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.2011.6.128.1.1.2.45.1.4", 65535)
            .await
            .unwrap_or_default();
        for vb in model_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                if let Some(ref m) = vb.value_str {
                    let cleaned = m.trim();
                    if !cleaned.is_empty() && cleaned != "N/A" {
                        model_map.insert(key, cleaned.to_string());
                    }
                }
            }
        }

        // 6. Mapeamento de Rx ONU (.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.4)
        let mut rx_map = std::collections::HashMap::new();
        let rx_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.2011.6.128.1.1.2.51.1.4", 65535)
            .await
            .unwrap_or_default();
        for vb in rx_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                let raw_val = vb.value_int.unwrap_or(0);
                if raw_val != 0 && raw_val != 2147483647 && raw_val != -1 {
                    let dbm = (raw_val as f64) / 100.0;
                    if dbm > -60.0 && dbm < 10.0 {
                        rx_map.insert(key, dbm);
                    }
                }
            }
        }

        // 7. Mapeamento de Tx ONU (.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.3)
        let mut tx_map = std::collections::HashMap::new();
        let tx_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.2011.6.128.1.1.2.51.1.3", 65535)
            .await
            .unwrap_or_default();
        for vb in tx_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                let raw_val = vb.value_int.unwrap_or(0);
                if raw_val != 0 && raw_val != 2147483647 && raw_val != -1 {
                    let dbm = (raw_val as f64) / 100.0;
                    if dbm > -20.0 && dbm < 20.0 {
                        tx_map.insert(key, dbm);
                    }
                }
            }
        }

        // 8. Mapeamento de OLT Rx Upstream (.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.6)
        let mut olt_rx_map = std::collections::HashMap::new();
        let olt_rx_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.2011.6.128.1.1.2.51.1.6", 65535)
            .await
            .unwrap_or_default();
        for vb in olt_rx_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                let raw_val = vb.value_int.unwrap_or(0);
                if raw_val != 0 && raw_val != 2147483647 && raw_val != -1 {
                    let dbm = (raw_val as f64 - 10000.0) / 100.0;
                    if dbm > -60.0 && dbm < 10.0 {
                        olt_rx_map.insert(key, dbm);
                    }
                }
            }
        }

        // 9. Mapeamento de Temperatura ONU (.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.1)
        let mut temp_map = std::collections::HashMap::new();
        let temp_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.2011.6.128.1.1.2.51.1.1", 65535)
            .await
            .unwrap_or_default();
        for vb in temp_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                let raw_val = vb.value_int.unwrap_or(0);
                if raw_val > 0 && raw_val < 150 {
                    temp_map.insert(key, raw_val as f64);
                }
            }
        }

        // 10. Mapeamento de Tensão / Voltagem (.1.3.6.1.4.1.2011.6.128.1.1.2.51.1.2)
        let mut volt_map = std::collections::HashMap::new();
        let volt_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.2011.6.128.1.1.2.51.1.2", 65535)
            .await
            .unwrap_or_default();
        for vb in volt_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                let raw_val = vb.value_int.unwrap_or(0);
                if raw_val > 0 && raw_val != 2147483647 {
                    volt_map.insert(key, (raw_val as f64) / 100.0);
                }
            }
        }

        // 11. Mapeamento de Causa da Última Queda (.1.3.6.1.4.1.2011.6.128.1.1.2.47.1.3)
        // 1: Dying Gasp, 2 ou 3: LOS, 13: manual_deactivate, 254: normal_down/los
        let mut down_cause_map = std::collections::HashMap::new();
        let down_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.2011.6.128.1.1.2.47.1.3", 65535)
            .await
            .unwrap_or_default();
        for vb in down_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                let raw_val = vb.value_int.unwrap_or(0);
                let reason = match raw_val {
                    1 => "dying_gasp",
                    2 | 3 => "los",
                    13 => "manual_deactivate",
                    _ => "los",
                };
                down_cause_map.insert(key, reason.to_string());
            }
        }

        // 12. Mapeamento de Distância Física da Fibra em Metros (.1.3.6.1.4.1.2011.6.128.1.1.2.46.1.20)
        let mut dist_map = std::collections::HashMap::new();
        let dist_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.2011.6.128.1.1.2.46.1.20", 65535)
            .await
            .unwrap_or_default();
        for vb in dist_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                let raw_val = vb.value_int.unwrap_or(-1);
                if raw_val > 0 && raw_val != 2147483647 {
                    dist_map.insert(key, raw_val as i32);
                }
            }
        }
        if !dist_map.is_empty() {
            info!(
                "Huawei '{}': Distâncias físicas da fibra mapeadas via SNMP puro para {} ONUs.",
                target.name,
                dist_map.len()
            );
        }

        for (idx, vb) in onu_table.iter().enumerate() {
            let raw_bytes = if !vb.value_raw.is_empty() {
                &vb.value_raw[..]
            } else if let Some(ref s) = vb.value_str {
                s.as_bytes()
            } else {
                &[]
            };

            let serial = if raw_bytes.len() >= 8 {
                let vendor_part = String::from_utf8_lossy(&raw_bytes[0..4]).to_string();
                let id_hex: String = raw_bytes[4..8]
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect();
                if vendor_part.chars().all(|c| c.is_alphanumeric()) {
                    format!("{}{}", vendor_part, id_hex)
                } else {
                    let full_hex: String = raw_bytes
                        .iter()
                        .take(8)
                        .map(|b| format!("{:02X}", b))
                        .collect();
                    format!("HWTC{}", &full_hex[full_hex.len().saturating_sub(8)..])
                }
            } else {
                format!("HWTC{:08X}", idx + 0x1000)
            };

            let (slot, pon_port, onu_id) = Self::parse_huawei_index(&vb.oid, idx);
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            let if_index_u32 = parts
                .get(parts.len().saturating_sub(2))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            let suffix = if parts.len() >= 2 {
                format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
            } else {
                format!("{}.{}", pon_port, onu_id)
            };

            let rx_dbm = rx_map.get(&suffix).copied();
            let tx_dbm = tx_map.get(&suffix).copied();
            let olt_rx_dbm = olt_rx_map.get(&suffix).copied();
            let olt_tx_dbm = olt_tx_map.get(&if_index_u32).copied();
            let temp_c = temp_map.get(&suffix).copied();
            let volt_v = volt_map.get(&suffix).copied();
            let customer_name = name_map.get(&suffix).cloned();
            let dist_m = dist_map.get(&suffix).copied();

            let is_online = rx_dbm.is_some() && rx_dbm.unwrap() > -45.0;
            let offline_reason = if !is_online {
                Some(
                    down_cause_map
                        .get(&suffix)
                        .cloned()
                        .unwrap_or_else(|| "los".to_string()),
                )
            } else {
                None
            };

            // Cálculo automático da atenuação óptica da fibra
            let attenuation_db = match (olt_tx_dbm, rx_dbm) {
                (Some(tx_olt), Some(rx_onu)) if rx_onu > -45.0 => {
                    let att = tx_olt - rx_onu;
                    if att >= 0.0 && att <= 45.0 {
                        Some(att)
                    } else {
                        None
                    }
                }
                _ => match (tx_dbm, olt_rx_dbm) {
                    (Some(tx_onu), Some(rx_olt)) if rx_olt > -45.0 => {
                        let att = tx_onu - rx_olt;
                        if att >= 0.0 && att <= 45.0 {
                            Some(att)
                        } else {
                            None
                        }
                    }
                    _ => None,
                },
            };

            results.push(OnuOpticalData {
                slot,
                pon_port,
                onu_id,
                serial_number: serial,
                customer_identifier: customer_name,
                rx_power_dbm: rx_dbm,
                tx_power_dbm: tx_dbm,
                olt_rx_power_dbm: olt_rx_dbm,
                olt_tx_power_dbm: olt_tx_dbm,
                attenuation_db,
                temperature_c: temp_c,
                voltage_v: volt_v,
                bias_current_ma: None,
                distance_meters: dist_m,
                is_online,
                offline_reason,
            });
        }

        // Deduplica entradas por interface física única (slot, pon_port, onu_id)
        let mut unique_map: std::collections::HashMap<(i32, i32, i32), OnuOpticalData> =
            std::collections::HashMap::new();
        for item in results {
            let key = (item.slot, item.pon_port, item.onu_id);
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
            "Coleta SNMP da OLT Huawei '{}' finalizada com {} ONUs processadas.",
            target.name,
            final_results.len()
        );
        Ok(final_results)
    }
}
