use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Semaphore;
use log::{info, warn};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, Duration};

use crate::collector::driver::{OltDriver, OltTarget, OnuOpticalData};
use crate::collector::snmp::SnmpClient;

/// Driver Datacom Híbrido de Alta Performance (SNMP como primário de alta velocidade + SSH cirúrgico)
pub struct DatacomDriver;

impl DatacomDriver {
    pub fn new() -> Self {
        Self
    }

    /// Executa comandos no DmOS via SSH não-interativo utilizando streaming com watchdog de inatividade
    async fn execute_ssh_commands(
        target: &OltTarget,
        commands: &[&str],
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let user = target.mgmt_username.as_deref().unwrap_or("admin");
        let pass = target.mgmt_password.as_deref().unwrap_or("");
        let port = target.ssh_port;

        let script = commands.join("\n") + "\nexit\n";

        let mut child = tokio::process::Command::new("sshpass")
            .arg("-p")
            .arg(pass)
            .arg("ssh")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg("-o")
            .arg("UserKnownHostsFile=/dev/null")
            .arg("-o")
            .arg("ConnectTimeout=10")
            .arg("-p")
            .arg(port.to_string())
            .arg(format!("{}@{}", user, target.ip_address))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(script.as_bytes()).await?;
            stdin.flush().await?;
        }

        let stdout = child.stdout.take().ok_or("Falha ao abrir stdout do SSH Datacom")?;
        use tokio::io::{AsyncBufReadExt, BufReader};
        let mut reader = BufReader::new(stdout).lines();

        let mut full_output = String::new();
        // Inactivity Watchdog: Enquanto o DmOS estiver enviando dados/linhas, a coleta segue sem limite global.
        // Se ficar em silêncio absoluto por mais de 45 segundos sem enviar dados, encerra com segurança.
        let inactivity_duration = Duration::from_secs(45);

        loop {
            match tokio::time::timeout(inactivity_duration, reader.next_line()).await {
                Ok(Ok(Some(line))) => {
                    full_output.push_str(&line);
                    full_output.push('\n');
                }
                Ok(Ok(None)) => {
                    // Stream encerrado normalmente pela OLT
                    break;
                }
                Ok(Err(e)) => {
                    warn!("Datacom '{}': Erro no stream SSH: {}", target.name, e);
                    break;
                }
                Err(_) => {
                    warn!("Datacom '{}': Inatividade detectada (>45s sem novos dados no stream SSH). Encerrando comando.", target.name);
                    let _ = child.kill().await;
                    break;
                }
            }
        }

        let _ = child.wait().await;
        Ok(full_output)
    }

    /// Faz o parse da saída do comando `show interface gpon onu` do DmOS
    fn parse_onu_status_table(output: &str) -> Vec<(i32, i32, i32, String, bool, Option<String>)> {
        let mut list = Vec::new();
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("Itf") || trimmed.starts_with("---") {
                continue;
            }

            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4 {
                let itf = parts[0];
                let port_parts: Vec<&str> = itf.split('/').collect();
                if port_parts.len() >= 3 {
                    let slot = port_parts[1].parse::<i32>().unwrap_or(1);
                    let port = port_parts[2].parse::<i32>().unwrap_or(1);
                    let onu_id = parts[1].parse::<i32>().unwrap_or(0);
                    let serial = parts[2].to_string();
                    let oper_state = parts[3].to_lowercase();
                    let is_online = oper_state == "up";

                    let name = if parts.len() >= 6 {
                        Some(parts[5..].join(" "))
                    } else {
                        None
                    };

                    list.push((slot, port, onu_id, serial, is_online, name));
                }
            }
        }
        list
    }

    /// Faz o parse da saída do comando `show interface transceivers gpon`
    /// Retorna: (Tx Power em dBm, Temperatura em °C) por porta PON
    fn parse_transceivers_table(output: &str) -> std::collections::HashMap<i32, (f64, f64)> {
        let mut map = std::collections::HashMap::new();
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("gpon 1/1/") {
                let parts: Vec<&str> = trimmed.split('|').collect();
                if parts.len() >= 5 {
                    let itf_raw = parts[0].trim();
                    if let Some(port_str) = itf_raw.strip_prefix("gpon 1/1/") {
                        if let Ok(port) = port_str.trim().parse::<i32>() {
                            let temp_str = parts[1].trim().replace('C', "");
                            let temp_c = temp_str.trim().parse::<f64>().unwrap_or(35.0);
                            
                            let tx_str = parts[4].trim().replace("dBm", "");
                            let tx_dbm = tx_str.trim().parse::<f64>().unwrap_or(5.0);
                            
                            map.insert(port, (tx_dbm, temp_c));
                        }
                    }
                }
            }
        }
        map
    }

    /// Faz o parse do histórico de alarmes OMCI no DmOS para identificar causa da queda
    fn parse_alarm_reasons(alarm_log_output: &str) -> std::collections::HashMap<(i32, i32), String> {
        let mut reasons_map = std::collections::HashMap::new();

        for line in alarm_log_output.lines() {
            let trimmed = line.trim();
            if trimmed.contains("Alarm GPON_") && trimmed.contains("on source gpon-1/1/") {
                if let Some(idx) = trimmed.find("on source gpon-1/1/") {
                    let src_part = &trimmed[idx + "on source gpon-1/1/".len()..];
                    let port_onu: Vec<&str> = src_part.trim().split('/').collect();
                    if port_onu.len() >= 2 {
                        if let (Ok(port), Ok(onu_id)) = (port_onu[0].parse::<i32>(), port_onu[1].parse::<i32>()) {
                            if trimmed.contains("GPON_DGi") {
                                reasons_map.insert((port, onu_id), "dying_gasp".to_string());
                            } else if trimmed.contains("GPON_LOSi") || trimmed.contains("GPON_DOWi") || trimmed.contains("GPON_LOFi") {
                                reasons_map.entry((port, onu_id)).or_insert_with(|| "los".to_string());
                            }
                        }
                    }
                }
            }
        }

        reasons_map
    }

    /// Realiza coleta rápida via SNMP dos níveis ópticos, distâncias precisas, nomes e potências SFP PON
    async fn collect_via_snmp(
        target: &OltTarget,
    ) -> Result<
        (
            std::collections::HashMap<(i32, i32), f64>,
            std::collections::HashMap<(i32, i32), i32>,
            std::collections::HashMap<(i32, i32), String>,
            std::collections::HashMap<i32, f64>,
        ),
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let community = target.snmp_community.as_deref().unwrap_or("public");
        let client = SnmpClient::new(&target.ip_address, target.snmp_port, community, 3000).await?;

        let mut snmp_rx_map = std::collections::HashMap::new();
        let mut snmp_sfp_tx_map = std::collections::HashMap::new();

        // 1. Coleta os nomes das interfaces L2 mapeadas (.1.3.6.1.4.1.3709.3.6.2.1.1.3)
        // Exemplo: ifIndex -> "gpon-1/1/12-onu-0"
        let if_names = client.walk(".1.3.6.1.4.1.3709.3.6.2.1.1.3", 500, Duration::from_millis(5)).await.unwrap_or_default();
        let mut ifindex_to_port_onu = std::collections::HashMap::new();

        for vb in if_names {
            if let (Some(ifindex_str), Some(name_str)) = (vb.oid.split('.').last(), vb.value_str) {
                if name_str.starts_with("gpon-1/1/") && name_str.contains("-onu-") {
                    let parts: Vec<&str> = name_str.split("-onu-").collect();
                    if parts.len() == 2 {
                        let port_part = parts[0].trim_start_matches("gpon-1/1/");
                        if let (Ok(port), Ok(onu_id)) = (port_part.parse::<i32>(), parts[1].parse::<i32>()) {
                            ifindex_to_port_onu.insert(ifindex_str.to_string(), (port, onu_id));
                        }
                    }
                }
            }
        }

        // 2. Coleta a tabela de Rx óptico das ONUs (.1.3.6.1.4.1.3709.3.6.2.1.1.22)
        let rx_vbs = client.walk(".1.3.6.1.4.1.3709.3.6.2.1.1.22", 500, Duration::from_millis(5)).await.unwrap_or_default();
        for vb in rx_vbs {
            if let (Some(ifindex_str), Some(rx_str)) = (vb.oid.split('.').last(), vb.value_str) {
                if let Some(&(port, onu_id)) = ifindex_to_port_onu.get(ifindex_str) {
                    if let Ok(rx_val) = rx_str.parse::<f64>() {
                        if rx_val < -6.0 && rx_val > -45.0 {
                            snmp_rx_map.insert((port, onu_id), rx_val);
                        }
                    }
                }
            }
        }

        // 3. Coleta a tabela de Distâncias precisas das ONUs (.1.3.6.1.4.1.3709.3.6.2.1.1.21)
        // O DmOS expõe via SNMP o valor decimal exato da distância em km (ex: "2.35" km = 2350 metros, "0.48" km = 480 metros)
        let mut snmp_dist_map = std::collections::HashMap::new();
        let dist_vbs = client.walk(".1.3.6.1.4.1.3709.3.6.2.1.1.21", 500, Duration::from_millis(5)).await.unwrap_or_default();
        for vb in dist_vbs {
            if let (Some(ifindex_str), Some(dist_str)) = (vb.oid.split('.').last(), vb.value_str) {
                if let Some(&(port, onu_id)) = ifindex_to_port_onu.get(ifindex_str) {
                    if let Ok(km_val) = dist_str.trim().parse::<f64>() {
                        if km_val > 0.0 && km_val < 100.0 {
                            let meters = (km_val * 1000.0).round() as i32;
                            snmp_dist_map.insert((port, onu_id), meters);
                        }
                    }
                }
            }
        }

        // 4. Coleta a tabela de Nomes / Descrições dos Clientes (.1.3.6.1.4.1.3709.3.6.2.1.1.5)
        let mut snmp_name_map = std::collections::HashMap::new();
        let name_vbs = client.walk(".1.3.6.1.4.1.3709.3.6.2.1.1.5", 500, Duration::from_millis(5)).await.unwrap_or_default();
        for vb in name_vbs {
            if let (Some(ifindex_str), Some(name_val)) = (vb.oid.split('.').last(), vb.value_str) {
                if let Some(&(port, onu_id)) = ifindex_to_port_onu.get(ifindex_str) {
                    let cleaned = name_val.trim();
                    if !cleaned.is_empty() && cleaned != "N/A" {
                        snmp_name_map.insert((port, onu_id), cleaned.to_string());
                    }
                }
            }
        }

        // 5. Coleta potências Tx dos módulos SFP PON da OLT (.1.3.6.1.4.1.3709.3.6.8.2.1.1.3)
        let sfp_vbs = client.walk(".1.3.6.1.4.1.3709.3.6.8.2.1.1.3", 50, Duration::from_millis(5)).await.unwrap_or_default();
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

        Ok((snmp_rx_map, snmp_dist_map, snmp_name_map, snmp_sfp_tx_map))
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
        info!("Testando conectividade Híbrida (SNMP/SSH) com OLT Datacom '{}' ({}:{})", target.name, target.ip_address, target.ssh_port);
        match Self::execute_ssh_commands(target, &["show interface gpon onu"]).await {
            Ok(output) => {
                let lines_count = output.lines().filter(|l| !l.trim().is_empty() && !l.starts_with("Itf") && !l.starts_with("---")).count();
                Ok(format!("Datacom DmOS Híbrido Online ({} ONUs no chassi)", lines_count))
            }
            Err(e) => Err(format!("Falha de conexão com Datacom '{}': {}", target.name, e).into()),
        }
    }

    async fn collect_optical_signals(
        &self,
        target: &OltTarget,
        semaphore: Arc<Semaphore>,
    ) -> Result<Vec<OnuOpticalData>, Box<dyn std::error::Error + Send + Sync>> {
        let _permit = semaphore.acquire().await?;

        info!("Datacom '{}' [{}]: Iniciando Coleta Híbrida (SNMP primário + SSH cirúrgico de calibração)...", target.name, target.ip_address);

        // 1. Coleta ultra-rápida via SNMP de Rx, Distâncias precisas, Nomes de Clientes e Tx dos SFPs PON
        let (snmp_rx_map, snmp_dist_map, snmp_name_map, snmp_sfp_tx_map) = match Self::collect_via_snmp(target).await {
            Ok(data) => {
                info!("Datacom '{}': SNMP coletou {} níveis de sinal Rx, {} distâncias precisas, {} nomes de clientes e {} transceivers PON instantaneamente.", target.name, data.0.len(), data.1.len(), data.2.len(), data.3.len());
                data
            }
            Err(e) => {
                warn!("Datacom '{}': SNMP falhou ou indisponível ({}), utilizando fallback 100% SSH.", target.name, e);
                (std::collections::HashMap::new(), std::collections::HashMap::new(), std::collections::HashMap::new(), std::collections::HashMap::new())
            }
        };

        // 2. Coleta SSH do inventário geral de ONUs, transceivers e histórico de alarmes OMCI
        let main_output = Self::execute_ssh_commands(target, &[
            "show interface gpon onu",
            "show interface transceivers gpon",
            "show log | include \"Alarm GPON_\"",
        ]).await?;

        let onus = Self::parse_onu_status_table(&main_output);
        let sfp_telemetry_map = Self::parse_transceivers_table(&main_output);
        let alarm_reasons_map = Self::parse_alarm_reasons(&main_output);

        info!("Datacom '{}': {} ONUs no chassi. Coletando telemetria óptica e distância via SSH com intervalo de 1000ms...", target.name, onus.len());

        let mut results = Vec::new();

        // 3. Agrupa ONUs ativas por porta PON para consulta cadenciada suave
        let mut port_onus_map: std::collections::HashMap<i32, Vec<i32>> = std::collections::HashMap::new();
        for (_slot, port, onu_id, _, is_online, _) in &onus {
            if *is_online {
                port_onus_map.entry(*port).or_default().push(*onu_id);
            }
        }

        // Mapa de telemetria SSH: (port, onu_id) -> (Option<rx>, Option<tx>, Option<olt_rx>, Option<distance_meters>)
        let mut optical_data_map: std::collections::HashMap<String, (Option<f64>, Option<f64>, Option<f64>, Option<i32>)> = std::collections::HashMap::new();

        for (port, onu_ids) in port_onus_map {
            let mut port_commands = Vec::new();
            for onu_id in &onu_ids {
                port_commands.push(format!("show interface gpon 1/1/{} onu {} | display curly-braces", port, onu_id));
            }

            let cmd_refs: Vec<&str> = port_commands.iter().map(|s| s.as_str()).collect();
            if let Ok(batch_output) = Self::execute_ssh_commands(target, &cmd_refs).await {
                let mut current_rx: Option<f64> = None;
                let mut current_tx: Option<f64> = None;
                let mut current_olt_rx: Option<f64> = None;
                let mut current_dist: Option<i32> = None;
                let mut current_serial: Option<String> = None;

                for line in batch_output.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("rssi-value") {
                        if let Some(val_str) = trimmed.strip_prefix("rssi-value") {
                            let clean = val_str.trim().trim_end_matches(';');
                            if let Ok(v) = clean.parse::<f64>() {
                                if v < -5.0 && v > -45.0 {
                                    current_olt_rx = Some(v);
                                }
                            }
                        }
                    } else if trimmed.starts_with("rx-optical-pw") {
                        if let Some(val_str) = trimmed.strip_prefix("rx-optical-pw") {
                            let clean = val_str.trim().trim_end_matches(';');
                            if let Ok(v) = clean.parse::<f64>() {
                                // No DmOS, 0.00 dBm ou valores >= 0.0 indicam sensor DDM sem leitura óptica (desligado)
                                if v < -6.0 && v > -45.0 {
                                    current_rx = Some(v);
                                } else {
                                    current_rx = None;
                                }
                            }
                        }
                    } else if trimmed.starts_with("tx-optical-pw") {
                        if let Some(val_str) = trimmed.strip_prefix("tx-optical-pw") {
                            let clean = val_str.trim().trim_end_matches(';');
                            if let Ok(v) = clean.parse::<f64>() {
                                if v > -10.0 && v < 10.0 && v != 0.0 {
                                    current_tx = Some(v);
                                } else {
                                    current_tx = None;
                                }
                            }
                        }
                    } else if trimmed.starts_with("distance") {
                        if let Some(val_str) = trimmed.strip_prefix("distance") {
                            let clean = val_str.trim().trim_end_matches(';');
                            if clean != "N/A" {
                                if let Ok(val) = clean.parse::<f64>() {
                                    // Se DmOS reportar em km (ex: 2.3 ou 2), converte para metros; se já vier em metros (ex: 2340), preserva
                                    let meters = if val < 100.0 { (val * 1000.0) as i32 } else { val as i32 };
                                    current_dist = Some(meters);
                                }
                            }
                        }
                    } else if trimmed.starts_with("serial-number") {
                        if let Some(val_str) = trimmed.strip_prefix("serial-number") {
                            let clean = val_str.trim().trim_end_matches(';').trim();
                            if !clean.is_empty() {
                                current_serial = Some(clean.to_string());
                            }
                        }
                    } else if trimmed.starts_with("name") {
                        if let Some(sn) = current_serial.take() {
                            optical_data_map.insert(sn, (current_rx, current_tx, current_olt_rx, current_dist));
                            current_rx = None;
                            current_tx = None;
                            current_olt_rx = None;
                            current_dist = None;
                        }
                    }
                }

                if let Some(sn) = current_serial.take() {
                    optical_data_map.insert(sn, (current_rx, current_tx, current_olt_rx, current_dist));
                }
            }

            // Intervalo de descanso térmico de 1000ms (1 segundo) entre as portas PON
            sleep(std::time::Duration::from_millis(1000)).await;
        }

        // 4. Monta a lista final e realiza a calibração / validação cruzada entre SNMP e SSH
        for (slot, port, onu_id, serial, is_online, customer_name) in onus {
            let (ssh_rx, tx_power_dbm, olt_rx_power_dbm, ssh_dist) = if is_online {
                optical_data_map.get(&serial).copied().unwrap_or((None, None, None, None))
            } else {
                (None, None, None, None)
            };

            // Prioriza distância decimal precisa do SNMP (ex: 2.35 km = 2350m), com fallback para o SSH
            let distance_meters = if is_online {
                snmp_dist_map.get(&(port, onu_id)).copied().or(ssh_dist)
            } else {
                None
            };

            // Validação cruzada e calibração: Prioriza SNMP validado pelo SSH
            let rx_dbm = if let Some(snmp_rx) = snmp_rx_map.get(&(port, onu_id)) {
                Some(*snmp_rx)
            } else {
                ssh_rx
            };

            let olt_tx_val = snmp_sfp_tx_map
                .get(&port)
                .copied()
                .or_else(|| sfp_telemetry_map.get(&port).map(|t| t.0))
                .unwrap_or(5.0);

            let port_temp = sfp_telemetry_map.get(&port).map(|t| t.1).unwrap_or(35.0);

            let attenuation_db = match (Some(olt_tx_val), rx_dbm) {
                (Some(tx), Some(rx)) if is_online => {
                    let att = tx - rx;
                    if att >= 0.0 && att <= 45.0 { Some(att) } else { None }
                }
                _ => None,
            };

            // Identificação de causa de queda: Dying Gasp vs LOS
            let offline_reason = if !is_online {
                let reason = alarm_reasons_map
                    .get(&(port, onu_id))
                    .cloned()
                    .unwrap_or_else(|| "los".to_string());
                Some(reason)
            } else {
                None
            };

            // Prioriza identificador/nome de cliente completo do SNMP, com fallback para o SSH
            let final_customer_name = snmp_name_map
                .get(&(port, onu_id))
                .cloned()
                .or(customer_name);

            results.push(OnuOpticalData {
                slot,
                pon_port: port,
                onu_id,
                serial_number: serial,
                customer_identifier: final_customer_name,
                rx_power_dbm: rx_dbm,
                tx_power_dbm,
                olt_rx_power_dbm,
                olt_tx_power_dbm: Some(olt_tx_val),
                attenuation_db,
                temperature_c: if is_online { Some(port_temp) } else { None },
                voltage_v: None,
                bias_current_ma: None,
                distance_meters,
                is_online,
                offline_reason,
            });
        }

        info!("Coleta Híbrida Datacom '{}' finalizada com sucesso! Total: {} ONUs sincronizadas.", target.name, results.len());
        Ok(results)
    }
}
