use async_trait::async_trait;
use log::info;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::Duration;

use crate::collector::driver::{OltDriver, OltTarget, OnuOpticalData};
use crate::collector::snmp::SnmpClient;

/// Driver Datacom 100% SNMPv2c de Alta Performance (DmOS >= 12.6 nativo, sem SSH)
pub struct DatacomDriver;

impl DatacomDriver {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl OltDriver for DatacomDriver {
    fn vendor_name(&self) -> &'static str {
        "datacom"
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
            .unwrap_or_else(|| "Datacom DmOS OLT".to_string());

        let hostname = client
            .get(".1.3.6.1.2.1.1.5.0")
            .await
            .ok()
            .flatten()
            .and_then(|vb| vb.value_str)
            .unwrap_or_default();

        Ok(format!(
            "Datacom SNMPv2c Online | Host: {} | {}",
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
            "Datacom '{}' [{}]: Iniciando varredura 100% SNMPv2c (DmOS >= 12.6)...",
            target.name, target.ip_address
        );

        // 1. Tabela de Nomes de Interfaces L2 / Mapeamento (.1.3.6.1.4.1.3709.3.6.2.1.1.3)
        // Exemplo: ifIndex (ex: 16777216) -> "gpon-1/1/1-onu-0"
        let if_names = client
            .walk(
                ".1.3.6.1.4.1.3709.3.6.2.1.1.3",
                1000,
                Duration::from_millis(5),
            )
            .await
            .unwrap_or_default();

        let mut ifindex_map: HashMap<String, (i32, i32, i32)> = HashMap::new(); // ifindex_str -> (slot, port, onu_id)

        for vb in if_names {
            if let (Some(ifindex_str), Some(name_str)) = (vb.oid.split('.').last(), vb.value_str) {
                if name_str.starts_with("gpon-") && name_str.contains("-onu-") {
                    let parts: Vec<&str> = name_str.split("-onu-").collect();
                    if parts.len() == 2 {
                        let onu_id = parts[1].parse::<i32>().unwrap_or(0);
                        let port_path = parts[0].trim_start_matches("gpon-");
                        let port_parts: Vec<&str> = port_path.split('/').collect();
                        if port_parts.len() >= 3 {
                            let slot = port_parts[1].parse::<i32>().unwrap_or(1);
                            let port = port_parts[2].parse::<i32>().unwrap_or(1);
                            ifindex_map.insert(ifindex_str.to_string(), (slot, port, onu_id));
                        }
                    }
                }
            }
        }

        info!(
            "Datacom '{}': {} interfaces de ONUs mapeadas via SNMP.",
            target.name,
            ifindex_map.len()
        );

        // 2. Potência Óptica Rx da ONU (.1.3.6.1.4.1.3709.3.6.2.1.1.22)
        let mut snmp_rx_map: HashMap<String, f64> = HashMap::new();
        let rx_vbs = client
            .walk(
                ".1.3.6.1.4.1.3709.3.6.2.1.1.22",
                1000,
                Duration::from_millis(5),
            )
            .await
            .unwrap_or_default();

        for vb in rx_vbs {
            if let (Some(ifindex_str), Some(rx_str)) = (vb.oid.split('.').last(), vb.value_str) {
                if let Ok(rx_val) = rx_str.trim().parse::<f64>() {
                    if rx_val < -6.0 && rx_val > -45.0 {
                        snmp_rx_map.insert(ifindex_str.to_string(), rx_val);
                    }
                }
            }
        }

        // 3. Potência Óptica Tx da ONU (onuIfOnuPowerTx .1.3.6.1.4.1.3709.3.6.2.1.1.21)
        let mut snmp_tx_map: HashMap<String, f64> = HashMap::new();
        let tx_vbs = client
            .walk(
                ".1.3.6.1.4.1.3709.3.6.2.1.1.21",
                1000,
                Duration::from_millis(5),
            )
            .await
            .unwrap_or_default();

        for vb in tx_vbs {
            if let (Some(ifindex_str), Some(tx_str)) = (vb.oid.split('.').last(), vb.value_str) {
                if let Ok(tx_val) = tx_str.trim().parse::<f64>() {
                    if tx_val > -10.0 && tx_val < 15.0 && tx_val != 0.0 {
                        snmp_tx_map.insert(ifindex_str.to_string(), tx_val);
                    }
                }
            }
        }

        // 4. Causa da Última Desconexão (onuIfLastDownReason .1.3.6.1.4.1.3709.3.6.2.1.1.31)
        let mut snmp_down_reason_map: HashMap<String, String> = HashMap::new();
        let down_reason_vbs = client
            .walk(
                ".1.3.6.1.4.1.3709.3.6.2.1.1.31",
                1000,
                Duration::from_millis(5),
            )
            .await
            .unwrap_or_default();

        for vb in down_reason_vbs {
            if let Some(ifindex_str) = vb.oid.split('.').last() {
                let reason = if let Some(val_str) = vb.value_str {
                    let s = val_str.to_lowercase();
                    if s.contains("dgi")
                        || s.contains("dying")
                        || s.contains("gasp")
                        || s.contains("power")
                    {
                        "dying_gasp".to_string()
                    } else if s.contains("losi")
                        || s.contains("los")
                        || s.contains("lofi")
                        || s.contains("dowi")
                    {
                        "los".to_string()
                    } else if s.contains("manual") || s.contains("admin") || s.contains("deact") {
                        "manual_deactivate".to_string()
                    } else {
                        "los".to_string()
                    }
                } else if let Some(val_int) = vb.value_int {
                    match val_int {
                        1 | 2 => "dying_gasp".to_string(),
                        3 | 4 | 5 => "los".to_string(),
                        6 => "manual_deactivate".to_string(),
                        _ => "los".to_string(),
                    }
                } else {
                    "los".to_string()
                };
                snmp_down_reason_map.insert(ifindex_str.to_string(), reason);
            }
        }

        // 5. Status Primário da ONU (onuIfPrimaryStatus .1.3.6.1.4.1.3709.3.6.2.1.1.37)
        let mut snmp_primary_status_map: HashMap<String, bool> = HashMap::new();
        let primary_status_vbs = client
            .walk(
                ".1.3.6.1.4.1.3709.3.6.2.1.1.37",
                1000,
                Duration::from_millis(5),
            )
            .await
            .unwrap_or_default();

        for vb in primary_status_vbs {
            if let Some(ifindex_str) = vb.oid.split('.').last() {
                let is_up = if let Some(val_int) = vb.value_int {
                    val_int == 1 || val_int == 2 // 1 = up/active, 2 = operational
                } else if let Some(val_str) = vb.value_str {
                    let s = val_str.to_lowercase();
                    s == "up" || s == "active" || s == "operational" || s == "online"
                } else {
                    false
                };
                snmp_primary_status_map.insert(ifindex_str.to_string(), is_up);
            }
        }

        // 6. Nomes / Descrições dos Clientes (.1.3.6.1.4.1.3709.3.6.2.1.1.5)
        let mut snmp_name_map: HashMap<String, String> = HashMap::new();
        let name_vbs = client
            .walk(
                ".1.3.6.1.4.1.3709.3.6.2.1.1.5",
                1000,
                Duration::from_millis(5),
            )
            .await
            .unwrap_or_default();

        for vb in name_vbs {
            if let (Some(ifindex_str), Some(name_val)) = (vb.oid.split('.').last(), vb.value_str) {
                let cleaned = name_val.trim();
                if !cleaned.is_empty() && cleaned != "N/A" {
                    snmp_name_map.insert(ifindex_str.to_string(), cleaned.to_string());
                }
            }
        }

        // 7. Números de Série Reais das ONUs (.1.3.6.1.4.1.3709.3.6.2.1.1.38 - onuIfSerialNumber)
        let mut snmp_serial_map: HashMap<String, String> = HashMap::new();
        let serial_vbs = client
            .walk(
                ".1.3.6.1.4.1.3709.3.6.2.1.1.38",
                1000,
                Duration::from_millis(5),
            )
            .await
            .unwrap_or_default();

        for vb in serial_vbs {
            if let (Some(ifindex_str), Some(serial_val)) = (vb.oid.split('.').last(), vb.value_str) {
                let cleaned = serial_val.trim();
                if !cleaned.is_empty() && cleaned != "N/A" {
                    snmp_serial_map.insert(ifindex_str.to_string(), cleaned.to_string());
                }
            }
        }

        // 8. Potência Tx dos módulos SFP PON da OLT (.1.3.6.1.4.1.3709.3.6.8.2.1.1.3)
        let mut snmp_sfp_tx_map: HashMap<i32, f64> = HashMap::new();
        let sfp_vbs = client
            .walk(
                ".1.3.6.1.4.1.3709.3.6.8.2.1.1.3",
                50,
                Duration::from_millis(5),
            )
            .await
            .unwrap_or_default();

        for vb in sfp_vbs {
            let parts: Vec<&str> = vb.oid.split('.').collect();
            if parts.len() >= 2 {
                if let Some(val_int) = vb.value_int {
                    if let Some(port_code_str) = parts.get(parts.len().saturating_sub(2)) {
                        if let Ok(port_code) = port_code_str.parse::<i64>() {
                            let port_num = (port_code - 101744640) as i32;
                            if (1..=16).contains(&port_num) {
                                snmp_sfp_tx_map.insert(port_num, val_int as f64 / 100.0);
                            }
                        }
                    }
                }
            }
        }

        let mut results = Vec::new();

        // 9. Montagem do inventário consolidado de ONUs
        for (ifindex_str, (slot, port, onu_id)) in &ifindex_map {
            let rx_power_dbm = snmp_rx_map.get(ifindex_str).copied();
            let tx_power_dbm = snmp_tx_map.get(ifindex_str).copied();

            // Status online: se tiver leitura Rx válida ou se o primaryStatus indicar UP
            let is_online = if let Some(up) = snmp_primary_status_map.get(ifindex_str) {
                *up
            } else {
                rx_power_dbm.is_some()
            };

            let olt_tx_val = snmp_sfp_tx_map.get(port).copied().unwrap_or(5.0);

            let attenuation_db = match (Some(olt_tx_val), rx_power_dbm) {
                (Some(tx), Some(rx)) if is_online => {
                    let att = tx - rx;
                    if (0.0..=45.0).contains(&att) {
                        Some(att)
                    } else {
                        None
                    }
                }
                _ => None,
            };

            let offline_reason = if !is_online {
                let reason = snmp_down_reason_map
                    .get(ifindex_str)
                    .cloned()
                    .unwrap_or_else(|| "los".to_string());
                Some(reason)
            } else {
                None
            };

            let customer_name = snmp_name_map.get(ifindex_str).cloned();

            // Serial Real lido via OID oficial .38 (com fallback determinístico caso vazio)
            let serial_number = snmp_serial_map
                .get(ifindex_str)
                .cloned()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("DACM{:02X}{:02X}{:04X}", slot, port, onu_id));

            results.push(OnuOpticalData {
                slot: *slot,
                pon_port: *port,
                onu_id: *onu_id,
                serial_number,
                customer_identifier: customer_name,
                rx_power_dbm,
                tx_power_dbm,
                olt_rx_power_dbm: None,
                olt_tx_power_dbm: Some(olt_tx_val),
                attenuation_db,
                temperature_c: None,
                voltage_v: None,
                bias_current_ma: None,
                distance_meters: None,
                is_online,
                offline_reason,
            });
        }

        info!(
            "Datacom '{}': Coleta 100% SNMPv2c finalizada com sucesso! Total: {} ONUs lidas.",
            target.name,
            results.len()
        );

        Ok(results)
    }
}
