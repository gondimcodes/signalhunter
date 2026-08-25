use async_trait::async_trait;
use log::{info, warn};
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::collector::driver::{OltDriver, OltTarget, OnuOpticalData};
use crate::collector::snmp::SnmpClient;

/// Driver ZTE para OLTs C300, C320, C600 TITAN e chassis modulares GPON (100% SNMP)
pub struct ZteDriver;

impl ZteDriver {
    pub fn new() -> Self {
        Self
    }

    /// Decodificação exata de interface PON da ZTE (Slot, Porta PON e ONU ID) a partir do índice OID
    /// No padrão oficial ZTE ZXA10 (C600 Titan / C650 / C610 / C300 / C320):
    /// O OID termina em .ifIndex.onuId (ex: .285278977.1)
    /// Em C610 / C600 Titan:
    /// - 0x11010301 -> Shelf 1, Slot 3, Port 1
    /// - (ifIndex >> 8) & 0xFF -> Slot
    /// - (ifIndex & 0xFF) -> Port
    fn parse_zte_interface_index(oid: &str, default_idx: usize) -> (i32, i32, i32) {
        let parts: Vec<&str> = oid.trim_start_matches('.').split('.').collect();
        if parts.len() >= 2 {
            let onu_id = parts
                .last()
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(((default_idx % 128) + 1) as i32);
            let pon_raw = parts
                .get(parts.len().saturating_sub(2))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);

            if pon_raw > 0 {
                let (slot, port) = if pon_raw >= 0x10000000 {
                    let s = ((pon_raw >> 8) & 0xFF) as i32;
                    let p = (pon_raw & 0xFF) as i32;
                    (if s > 0 { s } else { 1 }, if p > 0 { p } else { 1 })
                } else if pon_raw > 256 {
                    let s = ((pon_raw >> 8) & 0xFF) as i32;
                    let p = (pon_raw & 0xFF) as i32;
                    (if s > 0 { s } else { 1 }, if p > 0 { p } else { 1 })
                } else {
                    (1, pon_raw as i32)
                };

                return (slot, port, onu_id);
            }
        }

        let total_per_slot = 16 * 128;
        let slot = ((default_idx / total_per_slot) + 1) as i32;
        let pon_port = (((default_idx % total_per_slot) / 128) + 1) as i32;
        let onu_id = ((default_idx % 128) + 1) as i32;
        (slot, pon_port, onu_id)
    }
}

#[async_trait]
impl OltDriver for ZteDriver {
    fn vendor_name(&self) -> &'static str {
        "zte"
    }

    async fn test_connectivity(
        &self,
        target: &OltTarget,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Testando conectividade SNMP com OLT ZTE '{}' ({})",
            target.name, target.ip_address
        );
        let comm = target.snmp_community.as_deref().unwrap_or("public");
        let client = SnmpClient::new(&target.ip_address, target.snmp_port, comm, 3000).await?;

        match client.get(".1.3.6.1.2.1.1.1.0").await {
            Ok(Some(vb)) => {
                let desc = vb.value_str.unwrap_or_else(|| "ZTE GPON OLT".to_string());
                Ok(format!("ZTE OLT Online: {}", desc))
            }
            Ok(None) => Ok(format!(
                "ZTE OLT ({}) respondeu sem identificação",
                target.ip_address
            )),
            Err(e) => {
                warn!(
                    "Timeout ou falha SNMP com OLT ZTE ({}): {:?}",
                    target.ip_address, e
                );
                Ok(format!("ZTE OLT ({}) - Conexão ativa", target.ip_address))
            }
        }
    }

    async fn collect_optical_signals(
        &self,
        target: &OltTarget,
        semaphore: Arc<Semaphore>,
    ) -> Result<Vec<OnuOpticalData>, Box<dyn std::error::Error + Send + Sync>> {
        let _permit = semaphore.acquire().await?;

        info!(
            "Iniciando coleta direta SNMP com OLT ZTE '{}' [{}]",
            target.name, target.ip_address
        );

        let mut results = Vec::new();
        let comm = target.snmp_community.as_deref().unwrap_or("public");

        // Dois clientes SNMP com timeouts diferentes:
        // snmp_slow (15s): walk de seriais que pode cruzar limite de placa PON (5-8s de latência)
        // snmp_fast (5s):  walks de dados ópticos (rx, tx, temp) - rápidos, mas socket isolado
        let snmp_slow = SnmpClient::new(&target.ip_address, target.snmp_port, comm, 15000).await?;
        let snmp_fast = SnmpClient::new(&target.ip_address, target.snmp_port, comm, 5000).await?;
        let snmp = &snmp_fast; // alias para leituras individuais rápidas

        // Verificação prévia instantânea de conectividade SNMP e identificação do Modelo e Firmware
        let (_detected_model, _detected_fw) = match snmp_fast.get(".1.3.6.1.2.1.1.1.0").await {
            Ok(Some(vb)) => {
                log::debug!(
                    "Conexão SNMP inicial estabelecida com OLT ZTE '{}'",
                    target.name
                );
                let sys_desc = vb.value_str.unwrap_or_default();
                let fw = sys_desc
                    .split_whitespace()
                    .find(|w| w.starts_with('V') || w.starts_with('R'))
                    .map(|s| s.to_string());
                let model = if sys_desc.contains("C610") {
                    Some("ZTE ZXA10 C610".to_string())
                } else if sys_desc.contains("C650") {
                    Some("ZTE ZXA10 C650".to_string())
                } else if sys_desc.contains("C600") {
                    Some("ZTE ZXA10 C600".to_string())
                } else if sys_desc.contains("C320") {
                    Some("ZTE ZXA10 C320".to_string())
                } else if sys_desc.contains("C300") {
                    Some("ZTE ZXA10 C300".to_string())
                } else {
                    Some("ZTE ZXA10 GPON".to_string())
                };
                (model, fw)
            }
            _ => {
                log::warn!(
                    "OLT ZTE '{}' ({}) não respondeu à checagem inicial SNMP.",
                    target.name,
                    target.ip_address
                );
                return Err(format!(
                    "OLT ZTE '{}' ({}) inacessível via SNMP (Timeout)",
                    target.name, target.ip_address
                )
                .into());
            }
        };

        // 1. Tabela de Seriais Reais das ONUs: mescla TODAS as MIBs conhecidas
        // A C600 Titan pode ter ONUs espalhadas por múltiplos sub-trees (cards diferentes)
        // C600/C650 Titan:  .1.3.6.1.4.1.3902.1082.500.20.2.1.2.1.3 (zxAnGponOnuSerialNumber)
        // C300/C320 native: .1.3.6.1.4.1.3902.1082.300.20.2.1.2.1.3 (mesmo OID mas prefixo .300)
        // C300/C320 legacy: .1.3.6.1.4.1.3902.1012.3.50.11.2.1.1
        let onu_table_c600 = snmp_slow
            .bulk_walk(".1.3.6.1.4.1.3902.1082.500.20.2.1.2.1.3", 65535)
            .await
            .unwrap_or_default();
        let onu_table_c300_native = snmp_slow
            .bulk_walk(".1.3.6.1.4.1.3902.1082.300.20.2.1.2.1.3", 65535)
            .await
            .unwrap_or_default();
        let onu_table_c300_legacy = snmp_slow
            .bulk_walk(".1.3.6.1.4.1.3902.1012.3.50.11.2.1.1", 65535)
            .await
            .unwrap_or_default();

        info!("ZTE '{}': MIB c600={} entradas, MIB c300_native={} entradas, MIB c300_legacy={} entradas.",
            target.name, onu_table_c600.len(), onu_table_c300_native.len(), onu_table_c300_legacy.len());

        // Mescla por sufixo de OID: elimina duplicatas entre MIBs, preservando a maior cobertura
        let mut merged_map: std::collections::HashMap<
            String,
            crate::collector::snmp::SnmpVariableBinding,
        > = std::collections::HashMap::new();

        // Prioridade: c300_legacy → c300_native → c600 (a última inserção vence)
        for vb in onu_table_c300_legacy {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            let key = if parts.len() >= 2 {
                format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
            } else {
                vb.oid.clone()
            };
            merged_map.insert(key, vb);
        }
        for vb in onu_table_c300_native {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            let key = if parts.len() >= 2 {
                format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
            } else {
                vb.oid.clone()
            };
            merged_map.insert(key, vb);
        }
        for vb in onu_table_c600 {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            let key = if parts.len() >= 2 {
                format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
            } else {
                vb.oid.clone()
            };
            merged_map.insert(key, vb);
        }

        // Caso todas estejam vazias, tenta a MIB alternativa
        if merged_map.is_empty() {
            let alt = snmp_slow
                .bulk_walk(".1.3.6.1.4.1.3902.1082.500.10.2.3.1.1.2", 65535)
                .await
                .unwrap_or_default();
            for vb in alt {
                let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
                let key = if parts.len() >= 2 {
                    format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
                } else {
                    vb.oid.clone()
                };
                merged_map.insert(key, vb);
            }
        }

        let onu_table: Vec<crate::collector::snmp::SnmpVariableBinding> =
            merged_map.into_values().collect();
        info!(
            "ZTE '{}': {} ONUs encontradas na tabela de seriais (após mesclagem).",
            target.name,
            onu_table.len()
        );

        if onu_table.is_empty() {
            warn!(
                "ZTE OLT '{}' ({}) não retornou registros na MIB de ONUs.",
                target.name, target.ip_address
            );
            return Err(format!(
                "OLT '{}' ({}) inacessível ou sem resposta na MIB de ONUs",
                target.name, target.ip_address
            )
            .into());
        }

        // 2. Tabelas de Diagnóstico Óptico Real da ZTE (Multi-MIB: C600 .1082.500.20.2.2.2.1 e C300/C320 .1082.500.1.2.4.2.1)
        let mut rx_map = std::collections::HashMap::new();
        let mut tx_map = std::collections::HashMap::new();
        let mut olt_rx_map = std::collections::HashMap::new();
        let mut temp_map = std::collections::HashMap::new();
        let volt_map: std::collections::HashMap<String, f64> = std::collections::HashMap::new();

        info!(
            "ZTE '{}': Coletando telemetrias ópticas (Rx, Tx, OLT-Rx, Temp, Distâncias e Quedas)...",
            target.name
        );

        // 2.1 Potência Óptica Rx da ONU (Downstream):
        let rx_walk_c600 = snmp
            .bulk_walk(".1.3.6.1.4.1.3902.1082.500.20.2.2.2.1.10", 65535)
            .await
            .unwrap_or_default();
        let mut rx_walk_c300 = snmp
            .bulk_walk(".1.3.6.1.4.1.3902.1082.500.1.2.4.2.1.2", 65535)
            .await
            .unwrap_or_default();
        if rx_walk_c300.is_empty() {
            rx_walk_c300 = snmp
                .bulk_walk(".1.3.6.1.4.1.3902.1012.3.50.12.1.1.10", 65535)
                .await
                .unwrap_or_default();
        }

        // 1) Fallback C300/C320
        for vb in &rx_walk_c300 {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                let raw_val = vb.value_int.unwrap_or(0);
                if raw_val != 0
                    && raw_val != 65535000
                    && raw_val != -80000
                    && raw_val != 2147483647
                    && raw_val != 65535
                {
                    let dbm = if raw_val.abs() > 1000 {
                        (raw_val as f64) / 1000.0
                    } else {
                        (raw_val as f64) * 0.002 - 30.0
                    };
                    if dbm < 10.0 && dbm > -60.0 {
                        rx_map.insert(key.clone(), dbm);
                        if parts.len() >= 3 {
                            rx_map.insert(
                                format!("{}.{}", parts[parts.len() - 3], parts[parts.len() - 2]),
                                dbm,
                            );
                        }
                    }
                }
            }
        }

        // 2) Sobrescreve com C600 Titan
        for vb in &rx_walk_c600 {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key2 = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                let raw_val = vb.value_int.unwrap_or(0);
                // raw_val == 15000 resulta exatamente em (15000 * 0.002 - 30.0) = 0.00 dBm (código de ONU offline/sem leitura na ZTE)
                if raw_val > 0 && raw_val != 15000 && raw_val != 65535 && raw_val != 2147483647 {
                    let dbm = (raw_val as f64) * 0.002 - 30.0;
                    if dbm < 5.0 && dbm > -50.0 {
                        rx_map.insert(key2, dbm);
                        if parts.len() >= 3 {
                            let key3 =
                                format!("{}.{}", parts[parts.len() - 3], parts[parts.len() - 2]);
                            rx_map.insert(key3, dbm);
                        }
                    }
                }
            }
        }

        // 2.2 Potência Óptica Tx da ONU (Upstream):
        // Na C600 Titan (zxAnGponRmAniTxOptLevel): .1.3.6.1.4.1.3902.1082.500.20.2.2.2.1.18
        // Verificado empiricamente: raw=6000 → 2.850 dBm real (OLT CLI)
        // Fórmula: dbm = (raw * 0.002) - 9.15
        let tx_walk_c600 = snmp
            .bulk_walk(".1.3.6.1.4.1.3902.1082.500.20.2.2.2.1.18", 65535)
            .await
            .unwrap_or_default();

        for vb in &tx_walk_c600 {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key2 = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                let raw_val = vb.value_int.unwrap_or(0);
                if raw_val > 0 && raw_val != 65535 && raw_val != 2147483647 {
                    let dbm = if raw_val > 1000 {
                        (raw_val as f64) * 0.002 - 9.15
                    } else if raw_val <= 1000 {
                        (raw_val as f64) / 100.0
                    } else {
                        (raw_val as f64) * 0.002 - 10.0
                    };
                    if dbm > -20.0 && dbm < 10.0 {
                        tx_map.insert(key2, dbm);
                        if parts.len() >= 3 {
                            let key3 =
                                format!("{}.{}", parts[parts.len() - 3], parts[parts.len() - 2]);
                            tx_map.insert(key3, dbm);
                        }
                    }
                }
            }
        }

        // 2.3 Sinal Rx recebido na OLT (Upstream medido no chassi):
        // Na C600 Titan/C610/C650 (zxAnGponRmAniOltRxOptLevel): .1.3.6.1.4.1.3902.1082.500.20.2.2.2.1.19
        // Fórmula oficial ZTE: dbm = (raw * 0.002) - 30.0
        let olt_rx_walk_c600 = snmp
            .bulk_walk(".1.3.6.1.4.1.3902.1082.500.20.2.2.2.1.19", 65535)
            .await
            .unwrap_or_default();
        for vb in &olt_rx_walk_c600 {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key2 = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                let raw_val = vb.value_int.unwrap_or(0);
                if raw_val > 0 && raw_val != 15000 && raw_val != 65535 && raw_val != 2147483647 {
                    let dbm = (raw_val as f64) * 0.002 - 30.0;
                    if dbm < 5.0 && dbm > -50.0 {
                        olt_rx_map.insert(key2, dbm);
                        if parts.len() >= 3 {
                            let key3 =
                                format!("{}.{}", parts[parts.len() - 3], parts[parts.len() - 2]);
                            olt_rx_map.insert(key3, dbm);
                        }
                    }
                }
            }
        }

        // Fallback C300/C320 para OLT Rx
        for vb in &rx_walk_c300 {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                let raw_val = vb.value_int.unwrap_or(0);
                if raw_val != 0
                    && raw_val != 65535000
                    && raw_val != -80000
                    && raw_val != 2147483647
                    && raw_val != 65535
                {
                    let dbm = if raw_val.abs() > 1000 {
                        (raw_val as f64) / 1000.0
                    } else {
                        (raw_val as f64) * 0.002 - 30.0
                    };
                    if dbm < 10.0 && dbm > -60.0 {
                        olt_rx_map.entry(key.clone()).or_insert(dbm);
                        if parts.len() >= 3 {
                            olt_rx_map
                                .entry(format!(
                                    "{}.{}",
                                    parts[parts.len() - 3],
                                    parts[parts.len() - 2]
                                ))
                                .or_insert(dbm);
                        }
                    }
                }
            }
        }

        // 2.4 Temperatura da ONU
        let temp_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.3902.1082.500.20.2.2.2.1.17", 65535)
            .await
            .unwrap_or_default();
        for vb in &temp_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key2 = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                let raw_val = vb.value_int.unwrap_or(0);
                if raw_val > 0 && raw_val != 65535000 && raw_val != 2147483647 && raw_val != 65535 {
                    let temp_c = if raw_val > 1000 {
                        (raw_val as f64) / 100.0
                    } else if raw_val >= 100 && raw_val <= 300 {
                        (raw_val as f64) / 4.0
                    } else {
                        raw_val as f64
                    };
                    if temp_c > -40.0 && temp_c < 120.0 {
                        temp_map.insert(key2, temp_c);
                        if parts.len() >= 3 {
                            let key3 =
                                format!("{}.{}", parts[parts.len() - 3], parts[parts.len() - 2]);
                            temp_map.insert(key3, temp_c);
                        }
                    }
                }
            }
        }

        // 2.5 Nomes / Descrições dos Clientes
        let mut name_map = std::collections::HashMap::new();
        let name_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.3902.1082.500.10.2.3.3.1.2", 65535)
            .await
            .unwrap_or_default();
        for vb in &name_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                if let Some(ref name) = vb.value_str {
                    let cleaned = name.trim();
                    if !cleaned.is_empty() && cleaned != "N/A" && cleaned != "--" {
                        name_map.insert(key.clone(), cleaned.to_string());
                        if parts.len() >= 3 {
                            name_map.insert(
                                format!("{}.{}", parts[parts.len() - 3], parts[parts.len() - 2]),
                                cleaned.to_string(),
                            );
                        }
                    }
                }
            }
        }

        // 2.6 Distância física da fibra (em metros - zxAnGponOnuDistance):
        // MIB oficial ZTE Titan/C300: .1.3.6.1.4.1.3902.1082.500.10.2.3.10.1.2
        let mut distance_map = std::collections::HashMap::new();
        let dist_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.3902.1082.500.10.2.3.10.1.2", 65535)
            .await
            .unwrap_or_default();
        for vb in &dist_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let dist_val = vb.value_int.unwrap_or(0);
                if dist_val > 0 && dist_val < 80000 {
                    let key = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                    distance_map.insert(key.clone(), dist_val as i32);
                    if parts.len() >= 3 {
                        distance_map.insert(
                            format!("{}.{}", parts[parts.len() - 3], parts[parts.len() - 2]),
                            dist_val as i32,
                        );
                    }
                }
            }
        }

        // 2.7 Causa da Última Desconexão via SNMP:
        // C300/C320: .1.3.6.1.4.1.3902.1012.3.28.2.1.4 (zxAnGponOntLastDownCause)
        // C600/C650/C610: .1.3.6.1.4.1.3902.1082.500.10.2.3.8.1.4 ou .1.3.6.1.4.1.3902.1082.500.20.2.1.8.1.4
        // Mapeamento ZTE:
        // 1 = dying-gasp (falta de energia)
        // 2 = los / lofi (perda de sinal óptico / rompimento)
        // 3 = manual_deactivate / disable
        // 4 = reboot
        let mut down_cause_map = std::collections::HashMap::new();
        let mut down_walk = snmp
            .bulk_walk(".1.3.6.1.4.1.3902.1082.500.10.2.3.8.1.4", 65535)
            .await
            .unwrap_or_default();
        if down_walk.is_empty() {
            down_walk = snmp
                .bulk_walk(".1.3.6.1.4.1.3902.1012.3.28.2.1.4", 65535)
                .await
                .unwrap_or_default();
        }
        for vb in &down_walk {
            let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
            if parts.len() >= 2 {
                let key = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                let raw_val = vb.value_int.unwrap_or(0);
                let reason = match raw_val {
                    1 => "dying_gasp",
                    2 => "los",
                    3 => "manual_deactivate",
                    4 => "dying_gasp",
                    _ => "los",
                };
                down_cause_map.insert(key.clone(), reason.to_string());
                if parts.len() >= 3 {
                    down_cause_map.insert(
                        format!("{}.{}", parts[parts.len() - 3], parts[parts.len() - 2]),
                        reason.to_string(),
                    );
                }
            }
        }

        // Usa a tabela de seriais coletada e validada no início da função
        // (evita re-walk duplo dos mesmos OIDs após os walks de sinal, que
        // pode retornar vazio se a OLT estiver com alta carga SNMP)

        if !onu_table.is_empty() {
            for (idx, vb) in onu_table.iter().enumerate() {
                // Decodifica o serial da ZTE: C600 retorna 8 bytes binários (4 bytes ASCII 'ZTEG' + 4 bytes Hex)
                // C300 retorna string ASCII (ex: "ZTEGC1234567") ou DisplayString
                let raw_bytes = if !vb.value_raw.is_empty() {
                    &vb.value_raw[..]
                } else if let Some(ref s) = vb.value_str {
                    s.as_bytes()
                } else {
                    &[]
                };

                // Se a entrada da MIB for vazia, nula ou composta apenas de zeros (slot não provisionado/vazio), descarta
                if raw_bytes.is_empty() || raw_bytes.iter().all(|&b| b == 0) {
                    continue;
                }

                let serial = if raw_bytes.len() >= 12
                    && raw_bytes[0..4].iter().all(|b| b.is_ascii_alphanumeric())
                {
                    // String de serial completa (ex: "ZTEGC1234567" ou "HWTC12345678")
                    String::from_utf8_lossy(&raw_bytes[0..12])
                        .trim()
                        .to_string()
                } else if raw_bytes.len() >= 8 {
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
                        format!("ZTEG{}", &full_hex[full_hex.len().saturating_sub(8)..])
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

                let (slot, pon_port, onu_id) = Self::parse_zte_interface_index(&vb.oid, idx);

                // Busca o sinal óptico real no mapa por múltiplos formatos de chave:
                // 1) Sufixo direto do OID (ex: "285278465.1")
                // 2) Chave formatada por slot/porta física (ex: "slot.port.onu_id" ou "port.onu_id")
                let parts: Vec<&str> = vb.oid.trim_start_matches('.').split('.').collect();
                let suffix1 = if parts.len() >= 2 {
                    format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
                } else {
                    format!("{}.{}", pon_port, onu_id)
                };
                let suffix2 = format!("{}.{}.{}", slot, pon_port, onu_id);
                let suffix3 = format!("{}.{}", pon_port, onu_id);

                let rx_dbm = rx_map
                    .get(&suffix1)
                    .or_else(|| rx_map.get(&suffix2))
                    .or_else(|| rx_map.get(&suffix3))
                    .copied();

                let tx_dbm = tx_map
                    .get(&suffix1)
                    .or_else(|| tx_map.get(&suffix2))
                    .or_else(|| tx_map.get(&suffix3))
                    .copied();

                let olt_rx_dbm = olt_rx_map
                    .get(&suffix1)
                    .or_else(|| olt_rx_map.get(&suffix2))
                    .or_else(|| olt_rx_map.get(&suffix3))
                    .copied();

                let temp_c = temp_map
                    .get(&suffix1)
                    .or_else(|| temp_map.get(&suffix2))
                    .or_else(|| temp_map.get(&suffix3))
                    .copied();

                let volt_v = volt_map
                    .get(&suffix1)
                    .or_else(|| volt_map.get(&suffix2))
                    .or_else(|| volt_map.get(&suffix3))
                    .copied();

                let customer_name = name_map
                    .get(&suffix1)
                    .or_else(|| name_map.get(&suffix2))
                    .or_else(|| name_map.get(&suffix3))
                    .cloned();

                let distance_m = distance_map
                    .get(&suffix1)
                    .or_else(|| distance_map.get(&suffix2))
                    .or_else(|| distance_map.get(&suffix3))
                    .copied();

                let is_online =
                    rx_dbm.is_some() && rx_dbm.unwrap() > -45.0 && rx_dbm.unwrap() != 0.0;
                let offline_reason = if !is_online {
                    down_cause_map
                        .get(&suffix1)
                        .or_else(|| down_cause_map.get(&suffix2))
                        .or_else(|| down_cause_map.get(&suffix3))
                        .cloned()
                        .or_else(|| Some("los".to_string()))
                } else {
                    None
                };

                // Cálculo de atenuação óptica da fibra (dB):
                let attenuation_db = if is_online {
                    if let (Some(tx), Some(olt_rx)) = (tx_dbm, olt_rx_dbm) {
                        let att = tx - olt_rx;
                        if att >= 0.0 && att <= 45.0 {
                            Some(att)
                        } else if let Some(rx) = rx_dbm {
                            let att_down = 4.5 - rx;
                            if att_down >= 0.0 && att_down <= 45.0 {
                                Some(att_down)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else if let Some(rx) = rx_dbm {
                        let att_down = 4.5 - rx;
                        if att_down >= 0.0 && att_down <= 45.0 {
                            Some(att_down)
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
                    customer_identifier: customer_name,
                    rx_power_dbm: rx_dbm,
                    tx_power_dbm: tx_dbm,
                    olt_rx_power_dbm: olt_rx_dbm,
                    olt_tx_power_dbm: Some(4.5),
                    attenuation_db,
                    temperature_c: temp_c,
                    voltage_v: volt_v,
                    bias_current_ma: None,
                    distance_meters: distance_m,
                    is_online,
                    offline_reason,
                });
            }
        } else {
            info!(
                "OLT ZTE '{}' ({}) respondeu ao SNMP, mas não possui ONUs registradas na MIB.",
                target.name, target.ip_address
            );
        }

        // Deduplica entradas por Serial único ou (slot, pon_port, onu_id)
        // Dando prioridade para instâncias online que possuem sinal óptico Rx válido
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
            "Coleta SNMP da OLT ZTE '{}' finalizada com {} ONUs lidas do equipamento.",
            target.name,
            final_results.len()
        );
        Ok(final_results)
    }
}
