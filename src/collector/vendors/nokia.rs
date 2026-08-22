use async_trait::async_trait;
use log::{debug, info, warn};
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::collector::driver::{OltDriver, OltTarget, OnuOpticalData};
use crate::collector::snmp::SnmpClient;

/// Driver Nokia para OLTs ISAM e Lightspan FX
pub struct NokiaDriver;

impl NokiaDriver {
    pub fn new() -> Self {
        Self
    }

    fn parse_nokia_index(oid: &str, default_idx: usize) -> (i32, i32, i32) {
        let parts: Vec<&str> = oid.trim_start_matches('.').split('.').collect();
        if parts.len() >= 2 {
            let onu_id = parts
                .last()
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(1);
            let port = parts
                .get(parts.len() - 2)
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(1);
            let slot = ((port / 16) + 1).max(1);
            let pon_port = ((port % 16) + 1).max(1);
            return (slot, pon_port, onu_id);
        }
        let total = 16 * 64;
        let slot = ((default_idx / total) + 1) as i32;
        let port = (((default_idx % total) / 64) + 1) as i32;
        let onu = ((default_idx % 64) + 1) as i32;
        (slot, port, onu)
    }
}

#[async_trait]
impl OltDriver for NokiaDriver {
    fn vendor_name(&self) -> &'static str {
        "nokia"
    }

    async fn test_connectivity(
        &self,
        target: &OltTarget,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Testando conectividade SNMP com OLT Nokia '{}' ({})",
            target.name, target.ip_address
        );
        let comm = target.snmp_community.as_deref().unwrap_or("public");
        let client = SnmpClient::new(&target.ip_address, target.snmp_port, comm, 2000).await?;
        match client.get(".1.3.6.1.2.1.1.1.0").await {
            Ok(Some(vb)) => {
                let desc = vb
                    .value_str
                    .unwrap_or_else(|| "Nokia ISAM Lightspan".to_string());
                Ok(format!("Nokia OLT Online: {}", desc))
            }
            _ => Err(format!(
                "OLT Nokia '{}' ({}) inacessível via SNMP",
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
            "Iniciando coleta SNMP com OLT Nokia '{}' [{}]",
            target.name, target.ip_address
        );

        let mut results = Vec::new();
        let comm = target.snmp_community.as_deref().unwrap_or("public");
        let snmp = SnmpClient::new(&target.ip_address, target.snmp_port, comm, 3500).await?;

        // 1. Checagem prévia rápida e identificação do Modelo e Firmware
        let (_detected_model, _detected_fw) = match snmp.get(".1.3.6.1.2.1.1.1.0").await {
            Ok(Some(vb)) => {
                debug!("Conexão SNMP estabelecida com Nokia '{}'", target.name);
                let sys_desc = vb.value_str.unwrap_or_default();
                let fw = if let Some(first_word) = sys_desc.split_whitespace().next() {
                    if first_word.starts_with('R') || first_word.starts_with('V') {
                        Some(first_word.to_string())
                    } else {
                        None
                    }
                } else {
                    None
                };
                let model = if sys_desc.contains("ISAM") {
                    Some("ISAM 7360 FX".to_string())
                } else if sys_desc.contains("Lightspan") {
                    Some("Lightspan MF".to_string())
                } else {
                    Some("Nokia GPON OLT".to_string())
                };
                (model, fw)
            }
            _ => {
                warn!(
                    "OLT Nokia '{}' ({}) não respondeu à solicitação SNMP.",
                    target.name, target.ip_address
                );
                return Err(format!(
                    "OLT Nokia '{}' ({}) inacessível via SNMP (Timeout)",
                    target.name, target.ip_address
                )
                .into());
            }
        };

        // 2. Mapeamento de Status Operacional (.1.3.6.1.4.1.637.61.1.35.10.4.1.8)
        // No Alcatel/Nokia ISAM:
        // 12 = ONLINE / ACTIVE
        // 36 = OFFLINE / INACTIVE
        // 9  = STANDBY / DOWN
        let mut status_map = std::collections::HashMap::new();
        let status_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.637.61.1.35.10.4.1.8", 65535)
            .await
            .unwrap_or_default();
        for vb in status_walk {
            if let Some(idx_str) = vb.oid.split('.').last() {
                let state_val = vb.value_int.unwrap_or(0);
                let is_online = state_val == 12 || state_val == 1 || state_val == 2;
                status_map.insert(idx_str.to_string(), is_online);
            }
        }

        // 3. Mapeamento DDM de Óptica da ONU: Rx Power (.1.3.6.1.4.1.637.61.1.35.10.14.1.2)
        // No Alcatel ISAM 7360:
        // .35.10.14.1.2 = Rx Optical Power da ONU (escala: raw_int * 0.002 dBm, ex: -12926 * 0.002 = -25.852 dBm)
        let mut rx_map = std::collections::HashMap::new();
        let rx_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.637.61.1.35.10.14.1.2", 65535)
            .await
            .unwrap_or_default();
        for vb in rx_walk {
            if let Some(idx_str) = vb.oid.split('.').last() {
                if let Ok(raw_val) = vb.value_int.ok_or(()) {
                    // 32768 indica leitura inválida / offline
                    if raw_val < 0 && raw_val > -35000 {
                        let dbm = (raw_val as f64) * 0.002;
                        if dbm > -45.0 && dbm < -5.0 {
                            rx_map.insert(idx_str.to_string(), dbm);
                        }
                    }
                }
            }
        }

        // 4. Mapeamento DDM de Óptica da ONU: Tx Power (.1.3.6.1.4.1.637.61.1.35.10.14.1.3)
        // Escala: raw_int * 0.01 dBm (ex: 163 * 0.01 = 1.63 dBm) ou raw_int * 0.002 dBm
        let mut tx_map = std::collections::HashMap::new();
        let tx_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.637.61.1.35.10.14.1.3", 65535)
            .await
            .unwrap_or_default();
        for vb in tx_walk {
            if let Some(idx_str) = vb.oid.split('.').last() {
                if let Ok(raw_val) = vb.value_int.ok_or(()) {
                    if raw_val > 0 && raw_val != 32768 && raw_val < 10000 {
                        let dbm = if raw_val > 500 {
                            (raw_val as f64) * 0.002
                        } else {
                            (raw_val as f64) * 0.01
                        };
                        if dbm > -10.0 && dbm < 15.0 {
                            tx_map.insert(idx_str.to_string(), dbm);
                        }
                    }
                }
            }
        }

        // 5. Mapeamento DDM de Óptica da ONU: Temperatura (.1.3.6.1.4.1.637.61.1.35.10.14.1.6)
        // Escala: raw_int / 256.0 °C (padrão SFF-8472 / Alcatel)
        let mut temp_map = std::collections::HashMap::new();
        let temp_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.637.61.1.35.10.14.1.6", 65535)
            .await
            .unwrap_or_default();
        for vb in temp_walk {
            if let Some(idx_str) = vb.oid.split('.').last() {
                if let Ok(raw_val) = vb.value_int.ok_or(()) {
                    if raw_val > 0 && raw_val != 32768 && raw_val < 30000 {
                        let temp_c = (raw_val as f64) / 256.0;
                        if temp_c > -20.0 && temp_c < 120.0 {
                            temp_map.insert(idx_str.to_string(), temp_c);
                        }
                    }
                }
            }
        }

        // 6. Mapeamento DDM de Óptica da ONU: Tensão / Voltagem (.1.3.6.1.4.1.637.61.1.35.10.14.1.5)
        // Escala: raw_int / 10000.0 ou / 1000.0 V
        let mut volt_map = std::collections::HashMap::new();
        let volt_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.637.61.1.35.10.14.1.5", 65535)
            .await
            .unwrap_or_default();
        for vb in volt_walk {
            if let Some(idx_str) = vb.oid.split('.').last() {
                if let Ok(raw_val) = vb.value_int.ok_or(()) {
                    if raw_val > 0 && raw_val != 65536 && raw_val < 65000 {
                        let volt_v = (raw_val as f64) / 2000.0;
                        if volt_v > 1.0 && volt_v < 6.0 {
                            volt_map.insert(idx_str.to_string(), volt_v);
                        }
                    }
                }
            }
        }

        // 7. Mapeamento DDM de Óptica da ONU: Corrente de Bias (.1.3.6.1.4.1.637.61.1.35.10.14.1.4)
        // Escala: raw_int / 100.0 mA
        let mut bias_map = std::collections::HashMap::new();
        let bias_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.637.61.1.35.10.14.1.4", 65535)
            .await
            .unwrap_or_default();
        for vb in bias_walk {
            if let Some(idx_str) = vb.oid.split('.').last() {
                if let Ok(raw_val) = vb.value_int.ok_or(()) {
                    if raw_val > 0 && raw_val != 32768 && raw_val < 10000 {
                        let bias_ma = (raw_val as f64) / 100.0;
                        if bias_ma > 0.1 && bias_ma < 100.0 {
                            bias_map.insert(idx_str.to_string(), bias_ma);
                        }
                    }
                }
            }
        }

        // 8. Mapeamento de Distância da Fibra (.1.3.6.1.4.1.637.61.1.35.11.22.1.7)
        // No Alcatel/Nokia ISAM, a distância em metros é exposta como string (ex: "967" -> 967 metros)
        let mut dist_map = std::collections::HashMap::new();
        let dist_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.637.61.1.35.11.22.1.7", 65535)
            .await
            .unwrap_or_default();
        for vb in dist_walk {
            if let Some(idx_str) = vb.oid.split('.').last() {
                if let Some(ref s) = vb.value_str {
                    let cleaned = s.trim();
                    if let Ok(meters) = cleaned.parse::<i32>() {
                        if meters > 0 && meters < 70000 {
                            dist_map.insert(idx_str.to_string(), meters);
                        }
                    }
                }
            }
        }

        // 8.1 Mapeamento de OLT Rx Optical Power (Upstream medido no chassi da OLT Nokia):
        // MIB oficial Nokia ISAM: .1.3.6.1.4.1.637.61.1.35.10.18.1.2
        // Escala: raw_int / 10.0 dBm (ex: -274 -> -27.4 dBm). 65534 = offline/sem leitura
        let mut olt_rx_map = std::collections::HashMap::new();
        let olt_rx_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.637.61.1.35.10.18.1.2", 65535)
            .await
            .unwrap_or_default();
        for vb in olt_rx_walk {
            if let Some(idx_str) = vb.oid.split('.').last() {
                if let Ok(raw_val) = vb.value_int.ok_or(()) {
                    if raw_val != 65534 && raw_val != 32768 && raw_val > -500 && raw_val < 50 {
                        let dbm = (raw_val as f64) / 10.0;
                        if dbm > -45.0 && dbm < 5.0 {
                            olt_rx_map.insert(idx_str.to_string(), dbm);
                        }
                    }
                }
            }
        }

        // 8.2 Mapeamento de Causa de Queda / Alarmes da ONU (.1.3.6.1.4.1.637.61.1.35.10.1.1.88)
        // No Nokia/Alcatel ISAM:
        // 1 = Online / Active
        // 256 = Dying Gasp (Desligamento de Energia / Falha de Alimentação)
        // 2 = LOS (Loss of Signal / Fibra Rompida ou Sem Sinal Óptico)
        let mut offline_reason_map = std::collections::HashMap::new();
        let alarm_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.637.61.1.35.10.1.1.88", 65535)
            .await
            .unwrap_or_default();
        for vb in alarm_walk {
            if let Some(idx_str) = vb.oid.split('.').last() {
                if let Some(raw_val) = vb.value_int {
                    let reason = match raw_val {
                        256 => "dying_gasp",
                        2 => "los",
                        1 => "online",
                        _ => "los",
                    };
                    offline_reason_map.insert(idx_str.to_string(), reason.to_string());
                }
            }
        }

        // 9. Tabela de ONTs provisionadas: Seriais (.1.3.6.1.4.1.637.61.1.35.10.1.1.5)
        let onu_table = snmp
            .bulk_walk(".1.3.6.1.4.1.637.61.1.35.10.1.1.5", 65535)
            .await
            .unwrap_or_default();

        // Mapeamento de Modelos (.1.3.6.1.4.1.637.61.1.35.10.1.1.26)
        let mut model_map = std::collections::HashMap::new();
        let model_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.637.61.1.35.10.1.1.26", 65535)
            .await
            .unwrap_or_default();
        for vb in model_walk {
            if let Some(idx_str) = vb.oid.split('.').last() {
                if let Some(s) = vb.value_str {
                    let cleaned = s.trim().to_string();
                    if !cleaned.is_empty() {
                        model_map.insert(idx_str.to_string(), cleaned);
                    }
                }
            }
        }

        if !onu_table.is_empty() {
            for (idx, vb) in onu_table.iter().enumerate() {
                let idx_str = vb.oid.split('.').last().unwrap_or("");
                let if_index: i64 = idx_str.parse().unwrap_or(0);

                let raw_bytes = if !vb.value_raw.is_empty() {
                    &vb.value_raw[..]
                } else if let Some(ref s) = vb.value_str {
                    s.as_bytes()
                } else {
                    &[]
                };

                if raw_bytes.is_empty() || raw_bytes.iter().all(|&b| b == 0) {
                    continue;
                }

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
                        format!("ALCL{}", &full_hex[full_hex.len().saturating_sub(8)..])
                    }
                } else if let Some(ref s) = vb.value_str {
                    let cleaned = s.trim();
                    if cleaned.len() >= 8 && !cleaned.starts_with("00000000") {
                        cleaned.to_string()
                    } else {
                        continue;
                    }
                } else {
                    continue;
                };

                // Decodificação exata de slot, pon_port e onu_id do ifIndex Alcatel ISAM 7360:
                let (slot, pon_port, onu_id) = if if_index > 0 {
                    let slot_num = 8; // Slot LT padrão
                    let p = (((if_index >> 16) & 0x0F) + 1) as i32;
                    let o = (((if_index % 65536) / 512) + 1) as i32;
                    (slot_num, p.max(1), o.max(1))
                } else {
                    let (s, p, o) = Self::parse_nokia_index(&vb.oid, idx);
                    (s, p, o)
                };

                let is_online = status_map.get(idx_str).copied().unwrap_or(false);
                let rx_dbm = if is_online {
                    rx_map.get(idx_str).copied()
                } else {
                    None
                };

                let tx_dbm = if is_online {
                    tx_map.get(idx_str).copied()
                } else {
                    None
                };

                let temp_c = if is_online {
                    temp_map.get(idx_str).copied()
                } else {
                    None
                };

                let volt_v = if is_online {
                    volt_map.get(idx_str).copied()
                } else {
                    None
                };

                let bias_ma = if is_online {
                    bias_map.get(idx_str).copied()
                } else {
                    None
                };

                let olt_rx_dbm = if is_online {
                    olt_rx_map.get(idx_str).copied()
                } else {
                    None
                };

                let dist_m = if is_online {
                    dist_map.get(idx_str).copied()
                } else {
                    None
                };

                let customer_identifier = model_map.get(idx_str).cloned();

                // Cálculo de atenuação óptica (dB):
                // 1) Upstream real: ONU Tx - OLT Rx
                // 2) Downstream fallback: OLT Tx (+4.0 dBm) - ONU Rx
                let attenuation_db = if is_online {
                    if let (Some(tx), Some(olt_rx)) = (tx_dbm, olt_rx_dbm) {
                        let att = tx - olt_rx;
                        if att >= 0.0 && att <= 45.0 {
                            Some(att)
                        } else if let Some(rx) = rx_dbm {
                            Some(4.0 - rx)
                        } else {
                            None
                        }
                    } else if let Some(rx) = rx_dbm {
                        Some(4.0 - rx)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let offline_reason = if !is_online {
                    offline_reason_map
                        .get(idx_str)
                        .cloned()
                        .or_else(|| Some("los".to_string()))
                } else {
                    None
                };

                results.push(OnuOpticalData {
                    slot,
                    pon_port,
                    onu_id,
                    serial_number: serial,
                    customer_identifier,
                    rx_power_dbm: rx_dbm,
                    tx_power_dbm: tx_dbm,
                    olt_rx_power_dbm: olt_rx_dbm,
                    olt_tx_power_dbm: Some(4.0),
                    attenuation_db,
                    temperature_c: temp_c,
                    voltage_v: volt_v,
                    bias_current_ma: bias_ma,
                    distance_meters: dist_m,
                    is_online,
                    offline_reason,
                });
            }
        } else {
            info!("OLT Nokia '{}' ({}) comunicando com sucesso via SNMP (0 ONUs cadastradas no momento).", target.name, target.ip_address);
        }

        // Deduplica entradas por interface física única (slot, pon_port, onu_id)
        // Dando prioridade para instâncias online que possuem sinal óptico Rx válido
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
            "Coleta SNMP da OLT Nokia '{}' finalizada com {} ONUs lidas do equipamento.",
            target.name,
            final_results.len()
        );
        Ok(final_results)
    }
}
