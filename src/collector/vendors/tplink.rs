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
    /// OID format: ...<param_id>.<slot>.<port>.<onu_id> (ex: .26.0.2.0 -> slot 0, port 2, onu_id 0)
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

    /// Extrai a porta PON a partir de índices de porta (ex: 114689 -> port 1, ou sufixo .0.1 -> port 1)
    fn parse_pon_port(oid: &str) -> Option<i32> {
        let parts: Vec<&str> = oid.split('.').collect();
        if let Some(last) = parts.last().and_then(|s| s.parse::<u32>().ok()) {
            if last >= 114689 && last <= 114752 {
                return Some(((last - 114688) & 0xFF) as i32);
            }
            if last > 0 && last <= 64 {
                return Some(last as i32);
            }
        }
        if parts.len() >= 2 {
            if let Some(p) = parts[parts.len() - 1].parse::<i32>().ok() {
                if p > 0 && p <= 64 {
                    return Some(p);
                }
            }
        }
        None
    }

    /// Decodifica serial de ONU da TP-Link suportando ASCII DisplayString e Hex-STRING
    fn decode_serial(raw_bytes: &[u8], val_str: Option<&str>) -> Option<String> {
        if !raw_bytes.is_empty() && !raw_bytes.iter().all(|&b| b == 0) {
            // Se já for ASCII legível (ex: "TPLG12345678" ou "TPLG-EB130969")
            if raw_bytes.len() >= 8 && raw_bytes[0..4].iter().all(|b| b.is_ascii_alphanumeric()) {
                let s = String::from_utf8_lossy(raw_bytes).trim().to_string();
                if !s.is_empty() && s != "000000000000" && !s.chars().all(|c| c == '\0' || c == ' ')
                {
                    return Some(s);
                }
            }
            // Se for pacote binário de 8 bytes (4 bytes Vendor + 4 bytes Hex ID)
            if raw_bytes.len() >= 8 {
                let vendor_part = String::from_utf8_lossy(&raw_bytes[0..4]).to_string();
                let id_hex: String = raw_bytes[4..8]
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect();
                if vendor_part.chars().all(|c| c.is_alphanumeric()) {
                    return Some(format!("{}{}", vendor_part, id_hex));
                } else {
                    let full_hex: String = raw_bytes
                        .iter()
                        .take(8)
                        .map(|b| format!("{:02X}", b))
                        .collect();
                    return Some(format!(
                        "TPLG{}",
                        &full_hex[full_hex.len().saturating_sub(8)..]
                    ));
                }
            }
        }

        if let Some(s) = val_str {
            let trimmed = s.trim();
            // Caso venha como hex formatado por espaço: "54 50 4C 47 EB 13 09 69"
            if trimmed.contains(' ') && trimmed.len() >= 11 {
                let hex_parts: Vec<&str> = trimmed.split_whitespace().collect();
                let parsed_bytes: Vec<u8> = hex_parts
                    .iter()
                    .filter_map(|h| u8::from_str_radix(h, 16).ok())
                    .collect();
                if parsed_bytes.len() >= 8 {
                    return Self::decode_serial(&parsed_bytes, None);
                }
            }
            if !trimmed.is_empty()
                && trimmed != "000000000000"
                && !trimmed.chars().all(|c| c == '\0' || c == ' ')
            {
                return Some(trimmed.to_string());
            }
        }
        None
    }

    /// Converte valor de potência óptica para f64 suportando tanto DisplayString quanto Inteiro (centi-dBm)
    fn parse_power_value(val_str: Option<&str>, val_int: Option<i64>) -> Option<f64> {
        if let Some(s) = val_str {
            let trimmed = s
                .trim()
                .trim_end_matches("dBm")
                .trim_end_matches("dB")
                .trim();
            if !trimmed.is_empty()
                && trimmed != "--"
                && trimmed != "N/A"
                && trimmed != "null"
                && trimmed != "0"
                && trimmed != "0.0"
                && trimmed != "0.00"
            {
                if let Ok(val) = trimmed.parse::<f64>() {
                    if val > -50.0 && val < 20.0 {
                        return Some(val);
                    }
                }
            }
        }

        if let Some(num) = val_int {
            if num != 0 && num != -1 && num != 65535 && num != 2147483647 && num != -80000 {
                // Se o valor estiver em centi-dBm (ex: -2450 -> -24.50 dBm)
                let val = if num.abs() > 500 {
                    (num as f64) / 100.0
                } else {
                    num as f64
                };
                if val > -50.0 && val < 20.0 {
                    return Some(val);
                }
            }
        }

        None
    }

    /// Converte valor de temperatura para f64 suportando DisplayString e Inteiro
    fn parse_temp_value(val_str: Option<&str>, val_int: Option<i64>) -> Option<f64> {
        if let Some(s) = val_str {
            let trimmed = s
                .trim()
                .trim_end_matches('C')
                .trim_end_matches('c')
                .trim_end_matches("°C")
                .trim();
            if !trimmed.is_empty() && trimmed != "--" && trimmed != "N/A" {
                if let Ok(val) = trimmed.parse::<f64>() {
                    if val > -40.0 && val < 120.0 {
                        return Some(val);
                    }
                }
            }
        }

        if let Some(num) = val_int {
            if num > 0 && num != 65535 && num != 2147483647 {
                let val = if num > 500 {
                    (num as f64) / 100.0
                } else {
                    num as f64
                };
                if val > -40.0 && val < 120.0 {
                    return Some(val);
                }
            }
        }

        None
    }

    /// Converte valor de corrente de bias (mA) para f64 suportando DisplayString e Inteiro
    fn parse_bias_value(val_str: Option<&str>, val_int: Option<i64>) -> Option<f64> {
        if let Some(s) = val_str {
            let trimmed = s
                .trim()
                .trim_end_matches("mA")
                .trim_end_matches("ma")
                .trim();
            if !trimmed.is_empty() && trimmed != "--" && trimmed != "N/A" {
                if let Ok(val) = trimmed.parse::<f64>() {
                    if val >= 0.0 && val < 200.0 {
                        return Some(val);
                    }
                }
            }
        }

        if let Some(num) = val_int {
            if num >= 0 && num != 65535 && num != 2147483647 {
                let val = if num > 1000 {
                    (num as f64) / 1000.0
                } else if num > 200 {
                    (num as f64) / 100.0
                } else {
                    num as f64
                };
                if val >= 0.0 && val < 200.0 {
                    return Some(val);
                }
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

        // 1. Hostname padrão RFC 1213
        let hostname = client
            .get(".1.3.6.1.2.1.1.5.0")
            .await
            .ok()
            .flatten()
            .and_then(|vb| vb.value_str)
            .unwrap_or_default();

        // 2. Hardware Version / Modelo proprietário TP-Link (.1.3.6.1.4.1.11863.6.1.1.5.0)
        let hw_version = client
            .get(".1.3.6.1.4.1.11863.6.1.1.5.0")
            .await
            .ok()
            .flatten()
            .and_then(|vb| vb.value_str)
            .map(|s| s.trim().to_string());

        // 3. Firmware Version proprietário TP-Link (.1.3.6.1.4.1.11863.6.1.1.6.0)
        let fw_version = client
            .get(".1.3.6.1.4.1.11863.6.1.1.6.0")
            .await
            .ok()
            .flatten()
            .and_then(|vb| vb.value_str)
            .map(|s| s.trim().to_string());

        // 4. SysDescr de fallback
        let sys_descr = client
            .get(".1.3.6.1.2.1.1.1.0")
            .await?
            .and_then(|vb| vb.value_str)
            .unwrap_or_else(|| "TP-Link DeltaStream OLT".to_string());

        let model_display = hw_version
            .or_else(|| {
                if sys_descr.contains("DS-P") {
                    sys_descr
                        .split_whitespace()
                        .find(|w| w.starts_with("DS-P"))
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "TP-Link DeltaStream".to_string());

        let fw_display = fw_version.unwrap_or_else(|| "--".to_string());

        Ok(format!(
            "TP-Link SNMPv2c Online | Host: {} | Modelo: {} | Firmware: {}",
            if hostname.is_empty() {
                &target.name
            } else {
                &hostname
            },
            model_display,
            fw_display
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
                if let Some(sn) = Self::decode_serial(&vb.value_raw, vb.value_str.as_deref()) {
                    serial_map.insert(idx, sn);
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

        // ── 2. Potência Óptica Tx dos SFPs das Portas PON da OLT (.1.3.6.1.4.1.11863.6.96.1.7.1.1.5) ──
        let mut pon_sfp_tx_map: HashMap<i32, f64> = HashMap::new();
        if let Ok(pon_tx_vbs) = client
            .walk(
                ".1.3.6.1.4.1.11863.6.96.1.7.1.1.5",
                200,
                Duration::from_millis(5),
            )
            .await
        {
            for vb in pon_tx_vbs {
                if let Some(port) = Self::parse_pon_port(&vb.oid) {
                    if let Some(val) =
                        Self::parse_power_value(vb.value_str.as_deref(), vb.value_int)
                    {
                        pon_sfp_tx_map.insert(port, val);
                    }
                }
            }
        }

        // ── 3. Descrição / Nome do Cliente (omOnuDescription: .1.3.6.1.4.1.11863.6.100.1.7.2.1.5) ──
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

        // ── 4. Status Online / Offline (omOnlineStatus: .1.3.6.1.4.1.11863.6.100.1.7.2.1.11 com fallback para .41) ──
        let mut status_map: HashMap<(i32, i32, i32), bool> = HashMap::new();
        let mut status_vbs = client
            .walk(
                ".1.3.6.1.4.1.11863.6.100.1.7.2.1.11",
                1000,
                Duration::from_millis(5),
            )
            .await
            .unwrap_or_default();

        if status_vbs.is_empty() {
            // Fallback para OID de status operacional .41
            status_vbs = client
                .walk(
                    ".1.3.6.1.4.1.11863.6.100.1.7.2.1.41",
                    1000,
                    Duration::from_millis(5),
                )
                .await
                .unwrap_or_default();
        }

        for vb in status_vbs {
            if let Some(idx) = Self::parse_onu_index(&vb.oid) {
                // 1 = online, 0 = offline
                let is_on = vb.value_int.map(|v| v == 1).unwrap_or_else(|| {
                    vb.value_str
                        .as_deref()
                        .map(|s| {
                            s.trim() == "1"
                                || s.to_lowercase().contains("online")
                                || s.to_lowercase().contains("active")
                        })
                        .unwrap_or(false)
                });
                status_map.insert(idx, is_on);
            }
        }

        // ── 5. Potência Óptica Rx da ONU (omReceivedOpticalPower: .1.3.6.1.4.1.11863.6.100.1.7.2.1.26) ──
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
                    if let Some(val) =
                        Self::parse_power_value(vb.value_str.as_deref(), vb.value_int)
                    {
                        rx_map.insert(idx, val);
                    }
                }
            }
        }

        // ── 6. Potência Óptica Tx da ONU (omTransmittedOpticalPower: .1.3.6.1.4.1.11863.6.100.1.7.2.1.27) ──
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
                    if let Some(val) =
                        Self::parse_power_value(vb.value_str.as_deref(), vb.value_int)
                    {
                        tx_map.insert(idx, val);
                    }
                }
            }
        }

        // ── 7. Potência Óptica Rx na OLT (omOltReceivedOpticalPower: .1.3.6.1.4.1.11863.6.100.1.7.2.1.28) ──
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
                    if let Some(val) =
                        Self::parse_power_value(vb.value_str.as_deref(), vb.value_int)
                    {
                        olt_rx_map.insert(idx, val);
                    }
                }
            }
        }

        // ── 8. Distância Física (omDistance: .1.3.6.1.4.1.11863.6.100.1.7.2.1.18) ──
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

        // ── 9. Diagnósticos DDM (Temperatura, Voltagem, Corrente de Bias) ──
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
                    if let Some(val) = Self::parse_temp_value(vb.value_str.as_deref(), vb.value_int)
                    {
                        temp_map.insert(idx, val);
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
                    } else if let Some(s) = vb.value_str.as_deref() {
                        let trimmed = s.trim().trim_end_matches('V').trim_end_matches('v').trim();
                        if let Ok(v) = trimmed.parse::<f64>() {
                            if v > 0.0 && v < 10.0 {
                                volt_map.insert(idx, v);
                            }
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
                    if let Some(val) = Self::parse_bias_value(vb.value_str.as_deref(), vb.value_int)
                    {
                        bias_map.insert(idx, val);
                    }
                }
            }
        }

        // ── 10. Causa da Última Desconexão (omOnuLastDownCauses: .1.3.6.1.4.1.11863.6.100.1.7.2.1.42) ──
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

        // ── 11. Consolidação dos dados coletados ──
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

            // Potência Tx da porta PON da OLT (módulo SFP)
            let olt_tx = pon_sfp_tx_map.get(&pon_port).copied();

            // Cálculo da atenuação óptica real da fibra
            // Prioridade 1: Downstream (OLT SFP Tx - ONU Rx)
            // Prioridade 2: Upstream (ONU Tx - OLT Rx)
            let attenuation_db = match (olt_tx, rx) {
                (Some(tx_olt), Some(rx_onu)) if rx_onu > -45.0 => {
                    let att = tx_olt - rx_onu;
                    if att >= 0.0 && att <= 45.0 {
                        Some(att)
                    } else {
                        None
                    }
                }
                _ => match (tx, olt_rx) {
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
                serial_number,
                customer_identifier,
                rx_power_dbm: rx,
                tx_power_dbm: tx,
                olt_rx_power_dbm: olt_rx,
                olt_tx_power_dbm: olt_tx,
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
