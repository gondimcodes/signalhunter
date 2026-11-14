use crate::collector::driver::OnuOpticalData;
use crate::config::ThresholdsConfig;
use crate::db::queries::OnuRecord;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SignalClassification {
    Excellent,
    Good,
    Warning,
    Critical,
    Offline,
}

impl SignalClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Excellent => "excellent",
            Self::Good => "good",
            Self::Warning => "warning",
            Self::Critical => "critical",
            Self::Offline => "offline",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticIncident {
    pub id: String,
    pub severity: String, // "critical", "warning", "info"
    pub category: String, // "slot_failure", "pon_sfp_issue", "trunk_degradation", "saturation", "drop_isolated"
    pub title: String,
    pub olt_name: String,
    pub location: String, // "Slot 1", "Slot 1 / PON 6", "ONU ZTEG...", etc.
    pub total_affected_onus: usize,
    pub total_pon_onus: usize,
    pub avg_rx_dbm: f64,
    pub root_cause: String,
    pub recommended_action: String,
    pub confidence_score: u8, // 0 a 100%
    pub sample_onus: Vec<DiagnosticOnuSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticOnuSample {
    pub serial_number: String,
    pub customer: String,
    pub slot: i32,
    pub pon_port: i32,
    pub rx_power_dbm: Option<f64>,
    pub delta_db: Option<f64>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiagnosticSummary {
    pub total_incidents: usize,
    pub critical_incidents: usize,
    pub warning_incidents: usize,
    pub slot_incidents: usize,
    pub pon_incidents: usize,
    pub trunk_incidents: usize,
    pub incidents: Vec<DiagnosticIncident>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamIssueItem {
    pub onu_id: u64,
    pub olt_id: u64,
    pub olt_name: String,
    pub slot: i32,
    pub pon_port: i32,
    pub onu_id_num: i32,
    pub serial_number: String,
    pub customer_identifier: Option<String>,
    pub rx_power_dbm: Option<f64>,
    pub tx_power_dbm: Option<f64>,
    pub olt_rx_power_dbm: Option<f64>,
    pub olt_tx_power_dbm: Option<f64>,
    pub downstream_attenuation_db: Option<f64>,
    pub upstream_attenuation_db: Option<f64>,
    pub asymmetry_db: Option<f64>,
    pub issue_type: String, // "critical_upstream", "warning_upstream", "asymmetry", "both"
    pub root_cause: String,
    pub recommended_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PonUpstreamHealth {
    pub olt_name: String,
    pub slot: i32,
    pub pon_port: i32,
    pub total_onus: usize,
    pub saturated_upstream_count: usize,
    pub critical_upstream_count: usize,
    pub warning_upstream_count: usize,
    pub avg_olt_rx_dbm: Option<f64>,
    pub pon_status: String, // "saturated", "critical", "warning"
    pub is_sfp_receiver_suspect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpstreamDiagnosticsSummary {
    pub total_audited_onus: usize,
    pub saturated_upstream_count: usize,
    pub critical_upstream_count: usize,
    pub warning_upstream_count: usize,
    pub asymmetric_count: usize,
    pub suspect_pons_count: usize,
    pub items: Vec<UpstreamIssueItem>,
    pub suspect_pons: Vec<PonUpstreamHealth>,
}

pub struct OpticalEvaluator;

impl OpticalEvaluator {
    /// Classifica a qualidade do sinal óptico (Rx ONU)
    pub fn classify_rx_power(
        rx_dbm: Option<f64>,
        is_online: bool,
        _cfg: &ThresholdsConfig,
    ) -> SignalClassification {
        if !is_online {
            return SignalClassification::Offline;
        }

        let rx = match rx_dbm {
            Some(v) => v,
            None => return SignalClassification::Offline,
        };

        if (-18.0..=-14.0).contains(&rx) {
            SignalClassification::Excellent
        } else if (-23.0..-18.0).contains(&rx) {
            SignalClassification::Good
        } else if (-26.99..-23.0).contains(&rx) {
            SignalClassification::Warning
        } else {
            SignalClassification::Critical
        }
    }

    /// Calcula a degradação temporal comparando a leitura atual com a leitura anterior
    /// delta = curr - prev:
    /// Se delta < -3.0 dB (ex: era -17 dBm e foi para -22 dBm -> delta = -5 dB) -> Piora de Sinal
    /// Se delta > 0 (ex: era -38.87 dBm e foi para -34.23 dBm -> delta = +4.64 dB) -> Melhora de Sinal
    pub fn evaluate_degradation(
        current_rx_dbm: Option<f64>,
        previous_rx_dbm: Option<f64>,
        cfg: &ThresholdsConfig,
    ) -> (Option<f64>, bool) {
        match (current_rx_dbm, previous_rx_dbm) {
            (Some(curr), Some(prev)) => {
                // Descarta deltas caso alguma das leituras esteja fora do envelope físico operacional de GPON (-45 dBm a -8 dBm)
                if !(-45.0..=-8.0).contains(&prev) || !(-45.0..=-8.0).contains(&curr) {
                    return (None, false);
                }
                let delta = curr - prev;
                // Variações bruscas superiores a 15 dB decorrem de desconexão (LOS), troca de porta ou calibração
                if delta < -15.0 || delta > 15.0 {
                    return (Some((delta * 100.0).round() / 100.0), false);
                }
                // is_degraded quando a perda de potência óptica for maior ou igual ao threshold configurado (ex: delta <= -3.0 dB)
                let is_degraded = delta <= -cfg.degradation_alert_delta_db.abs();
                (Some((delta * 100.0).round() / 100.0), is_degraded)
            }
            _ => (None, false),
        }
    }

    /// Calcula a atenuação total do enlace (OLT Tx - ONU Rx)
    pub fn calculate_attenuation(data: &mut OnuOpticalData) {
        if let (Some(olt_tx), Some(onu_rx)) = (data.olt_tx_power_dbm, data.rx_power_dbm) {
            data.attenuation_db = Some(olt_tx - onu_rx);
        }
    }

    /// Motor Inteligente de Diagnóstico Óptico e Correlação de Falhas Raiz
    pub fn run_intelligent_diagnostics(onus: &[OnuRecord]) -> DiagnosticSummary {
        let mut summary = DiagnosticSummary::default();
        if onus.is_empty() {
            return summary;
        }

        // 1. Agrupa ONUs por OLT -> Slot -> PON Port
        let mut olt_tree: HashMap<String, HashMap<i32, HashMap<i32, Vec<&OnuRecord>>>> =
            HashMap::new();

        for onu in onus {
            let olt_name = onu
                .olt_name
                .clone()
                .unwrap_or_else(|| "OLT Principal".to_string());
            olt_tree
                .entry(olt_name)
                .or_default()
                .entry(onu.slot)
                .or_default()
                .entry(onu.pon_port)
                .or_default()
                .push(onu);
        }

        let mut incidents = Vec::new();

        for (olt_name, slots) in &olt_tree {
            for (&slot_id, pons) in slots {
                let mut slot_total_onus = 0;
                let mut slot_bad_onus = 0;
                let mut slot_bad_pons = 0;
                let total_pons_in_slot = pons.len();

                // Primeiro analisa cada porta PON individualmente
                for (&pon_id, pon_onus) in pons {
                    let total_in_pon = pon_onus.len();
                    slot_total_onus += total_in_pon;

                    let mut bad_onus_in_pon = Vec::new();
                    let mut critical_onus_in_pon = Vec::new();
                    let mut warning_onus_in_pon = Vec::new();
                    let mut degraded_onus_in_pon = Vec::new();
                    let mut saturated_onus_in_pon = Vec::new();
                    let mut sum_rx = 0.0;
                    let mut valid_rx_count = 0;

                    for onu in pon_onus {
                        if let Some(rx) = onu.latest_rx_power_dbm {
                            sum_rx += rx;
                            valid_rx_count += 1;

                            if rx > -14.0 {
                                saturated_onus_in_pon.push(*onu);
                            } else if rx < -27.0 {
                                critical_onus_in_pon.push(*onu);
                                bad_onus_in_pon.push(*onu);
                            } else if rx <= -23.01 {
                                warning_onus_in_pon.push(*onu);
                                bad_onus_in_pon.push(*onu);
                            }

                            if onu.is_degraded == Some(true)
                                || (onu.latest_delta_prev_rx_db.unwrap_or(0.0) <= -2.0)
                            {
                                degraded_onus_in_pon.push(*onu);
                            }
                        } else if onu.status == "offline" {
                            critical_onus_in_pon.push(*onu);
                            bad_onus_in_pon.push(*onu);
                        }
                    }

                    let avg_rx = if valid_rx_count > 0 {
                        sum_rx / (valid_rx_count as f64)
                    } else {
                        -99.0
                    };
                    slot_bad_onus += bad_onus_in_pon.len();

                    let bad_ratio = if total_in_pon > 0 {
                        (bad_onus_in_pon.len() as f64) / (total_in_pon as f64)
                    } else {
                        0.0
                    };
                    let deg_ratio = if total_in_pon > 0 {
                        (degraded_onus_in_pon.len() as f64) / (total_in_pon as f64)
                    } else {
                        0.0
                    };

                    let is_pon_collective_issue = (bad_ratio >= 0.30 && bad_onus_in_pon.len() >= 3)
                        || (bad_onus_in_pon.len() >= 10);
                    let is_trunk_collective_issue = (deg_ratio >= 0.15
                        && degraded_onus_in_pon.len() >= 2)
                        || (degraded_onus_in_pon.len() >= 5);

                    if is_pon_collective_issue {
                        slot_bad_pons += 1;
                    }

                    // A) Diagnóstico: Falha de SFP PON / Transceiver / Conector Sujo no DIO / Splitter Primário
                    if is_pon_collective_issue {
                        let sample_onus = bad_onus_in_pon
                            .iter()
                            .take(8)
                            .map(|o| DiagnosticOnuSample {
                                serial_number: o.serial_number.clone(),
                                customer: o
                                    .customer_identifier
                                    .clone()
                                    .or(o.custom_name.clone())
                                    .unwrap_or_else(|| "--".to_string()),
                                slot: o.slot,
                                pon_port: o.pon_port,
                                rx_power_dbm: o.latest_rx_power_dbm,
                                delta_db: o.latest_delta_prev_rx_db,
                                status: o.status.clone(),
                            })
                            .collect();

                        let confidence = (bad_ratio * 100.0).clamp(75.0, 99.0) as u8;

                        let mut breakdown_parts = Vec::new();
                        if !critical_onus_in_pon.is_empty() {
                            breakdown_parts
                                .push(format!("{} Crítica(s)/LOS", critical_onus_in_pon.len()));
                        }
                        if !warning_onus_in_pon.is_empty() {
                            breakdown_parts
                                .push(format!("{} em Atenção", warning_onus_in_pon.len()));
                        }
                        let breakdown_str = if !breakdown_parts.is_empty() {
                            format!(" ({})", breakdown_parts.join(" e "))
                        } else {
                            "".to_string()
                        };

                        let root_cause_msg = format!(
                            "{:.0}% das ONUs nesta PON (total: {} afetadas{}) apresentam sinal degradado com média geral de {:.2} dBm. Causa provável: Transceiver SFP com potência Tx baixa, conector óptico no DIO/GBIC sujo ou atenuação no splitter primário da porta.",
                            bad_ratio * 100.0,
                            bad_onus_in_pon.len(),
                            breakdown_str,
                            avg_rx
                        );

                        incidents.push(DiagnosticIncident {
                            id: format!("DIAG-PON-{}-{}-{}", olt_name, slot_id, pon_id),
                            severity: if bad_ratio >= 0.50 || critical_onus_in_pon.len() >= 5 { "critical".to_string() } else { "warning".to_string() },
                            category: "pon_sfp_issue".to_string(),
                            title: format!("Anomalia Coletiva na Porta PON S{}/P{}", slot_id, pon_id),
                            olt_name: olt_name.clone(),
                            location: format!("Slot {} / Porta PON {}", slot_id, pon_id),
                            total_affected_onus: bad_onus_in_pon.len(),
                            total_pon_onus: total_in_pon,
                            avg_rx_dbm: avg_rx,
                            root_cause: root_cause_msg,
                            recommended_action: "Inspecionar e limpar conector óptico do módulo SFP no DIO da central. Medir potência Tx com Power Meter antes do primeiro splitter.".to_string(),
                            confidence_score: confidence,
                            sample_onus,
                        });
                    }
                    // B) Diagnóstico: Degradação Coletiva de Rota / Microcurvatura no Tronco (Feeder)
                    else if is_trunk_collective_issue {
                        let sample_onus = degraded_onus_in_pon
                            .iter()
                            .take(8)
                            .map(|o| DiagnosticOnuSample {
                                serial_number: o.serial_number.clone(),
                                customer: o
                                    .customer_identifier
                                    .clone()
                                    .or(o.custom_name.clone())
                                    .unwrap_or_else(|| "--".to_string()),
                                slot: o.slot,
                                pon_port: o.pon_port,
                                rx_power_dbm: o.latest_rx_power_dbm,
                                delta_db: o.latest_delta_prev_rx_db,
                                status: o.status.clone(),
                            })
                            .collect();

                        incidents.push(DiagnosticIncident {
                            id: format!("DIAG-TRUNK-{}-{}-{}", olt_name, slot_id, pon_id),
                            severity: "warning".to_string(),
                            category: "trunk_degradation".to_string(),
                            title: format!("Degradação Súbita de Tronco Óptico na PON S{}/P{}", slot_id, pon_id),
                            olt_name: olt_name.clone(),
                            location: format!("Slot {} / Porta PON {}", slot_id, pon_id),
                            total_affected_onus: degraded_onus_in_pon.len(),
                            total_pon_onus: total_in_pon,
                            avg_rx_dbm: avg_rx,
                            root_cause: format!("{} ONUs ({:.0}%) desta PON sofreram perda súbita de potência óptica (ΔdB negativo) entre coletas. Causa provável: Microcurvatura, tração mecânica recente no cabo óptico alimentador ou atenuação em caixa de emenda (CEO).", degraded_onus_in_pon.len(), deg_ratio * 100.0),
                            recommended_action: "Realizar medição OTDR a partir do DIO para identificar o ponto exato da atenuação mecânica/curvatura na rota do cabo tronco.".to_string(),
                            confidence_score: 90,
                            sample_onus,
                        });
                    }
                    // C) Diagnóstico: Falhas Isoladas ou Grupos Menores de Drop / CTO na Porta PON
                    else {
                        // C.1) Se houver ONUs Críticas pontuais nesta PON:
                        for co in &critical_onus_in_pon {
                            let sample = vec![DiagnosticOnuSample {
                                serial_number: co.serial_number.clone(),
                                customer: co
                                    .customer_identifier
                                    .clone()
                                    .or(co.custom_name.clone())
                                    .unwrap_or_else(|| "--".to_string()),
                                slot: co.slot,
                                pon_port: co.pon_port,
                                rx_power_dbm: co.latest_rx_power_dbm,
                                delta_db: co.latest_delta_prev_rx_db,
                                status: co.status.clone(),
                            }];

                            let rx_desc = co
                                .latest_rx_power_dbm
                                .map(|rx| format!("{:.2} dBm (Crítico)", rx))
                                .unwrap_or_else(|| "Sem Sinal / LOS".to_string());

                            incidents.push(DiagnosticIncident {
                                id: format!("DIAG-CRIT-{}-{}", olt_name, co.serial_number),
                                severity: "critical".to_string(),
                                category: "drop_isolated".to_string(),
                                title: format!("Sinal Óptico Crítico na ONU {}", co.serial_number),
                                olt_name: olt_name.clone(),
                                location: format!("Slot {} / Porta PON {} (ONU #{})", co.slot, co.pon_port, co.onu_id),
                                total_affected_onus: 1,
                                total_pon_onus: total_in_pon,
                                avg_rx_dbm: co.latest_rx_power_dbm.unwrap_or(-35.0),
                                root_cause: format!("Assinante '{}' operando com potência óptica extrema de {} ou desconexão por Perda de Sinal (LOS). Causa provável: Drop óptico atenuado, conector SC-APC sujo/frouxo ou rompimento local na fibra do cliente.", co.customer_identifier.as_deref().unwrap_or(&co.serial_number), rx_desc),
                                recommended_action: "Enviar equipe técnica para vistoria no drop óptico do assinante: limpar conector na CTO/PTO, verificar raio de curvatura e medir atenuação com Power Meter.".to_string(),
                                confidence_score: 96,
                                sample_onus: sample,
                            });
                        }

                        // C.2) Se houver grupo de ONUs em Atenção preventiva (-23 a -27 dBm):
                        if !warning_onus_in_pon.is_empty() {
                            let sample_onus = warning_onus_in_pon
                                .iter()
                                .take(6)
                                .map(|o| DiagnosticOnuSample {
                                    serial_number: o.serial_number.clone(),
                                    customer: o
                                        .customer_identifier
                                        .clone()
                                        .or(o.custom_name.clone())
                                        .unwrap_or_else(|| "--".to_string()),
                                    slot: o.slot,
                                    pon_port: o.pon_port,
                                    rx_power_dbm: o.latest_rx_power_dbm,
                                    delta_db: o.latest_delta_prev_rx_db,
                                    status: o.status.clone(),
                                })
                                .collect();

                            let warn_avg = warning_onus_in_pon
                                .iter()
                                .filter_map(|o| o.latest_rx_power_dbm)
                                .sum::<f64>()
                                / (warning_onus_in_pon.len() as f64);

                            incidents.push(DiagnosticIncident {
                                id: format!("DIAG-WARN-{}-{}-{}", olt_name, slot_id, pon_id),
                                severity: "warning".to_string(),
                                category: "attenuation_warning".to_string(),
                                title: format!("Alerta Preventivo de Atenção na PON S{}/P{}", slot_id, pon_id),
                                olt_name: olt_name.clone(),
                                location: format!("Slot {} / Porta PON {}", slot_id, pon_id),
                                total_affected_onus: warning_onus_in_pon.len(),
                                total_pon_onus: total_in_pon,
                                avg_rx_dbm: warn_avg,
                                root_cause: format!("{} ONUs nesta porta PON operam na faixa de atenção (-23.01 a -27.00 dBm, média: {:.2} dBm). Risco de queda para estado crítico em caso de pequenas variações térmicas ou mecânicas na fibra.", warning_onus_in_pon.len(), warn_avg),
                                recommended_action: "Agendar revisão preventiva nos splitters secundários das CTOs vinculadas a esta porta PON e checar conectorização.".to_string(),
                                confidence_score: 85,
                                sample_onus,
                            });
                        }
                    }

                    // D) Diagnóstico: Saturação Óptica de Enlace Curto (> -14.00 dBm)
                    if !saturated_onus_in_pon.is_empty() {
                        let sample_onus = saturated_onus_in_pon
                            .iter()
                            .take(5)
                            .map(|o| DiagnosticOnuSample {
                                serial_number: o.serial_number.clone(),
                                customer: o
                                    .customer_identifier
                                    .clone()
                                    .or(o.custom_name.clone())
                                    .unwrap_or_else(|| "--".to_string()),
                                slot: o.slot,
                                pon_port: o.pon_port,
                                rx_power_dbm: o.latest_rx_power_dbm,
                                delta_db: o.latest_delta_prev_rx_db,
                                status: o.status.clone(),
                            })
                            .collect();

                        incidents.push(DiagnosticIncident {
                            id: format!("DIAG-SAT-{}-{}-{}", olt_name, slot_id, pon_id),
                            severity: "info".to_string(),
                            category: "saturation".to_string(),
                            title: format!("Sinal Saturado em Clientes Próximos na PON S{}/P{}", slot_id, pon_id),
                            olt_name: olt_name.clone(),
                            location: format!("Slot {} / Porta PON {}", slot_id, pon_id),
                            total_affected_onus: saturated_onus_in_pon.len(),
                            total_pon_onus: total_in_pon,
                            avg_rx_dbm: avg_rx,
                            root_cause: "ONUs recebendo potência superior a -14.00 dBm por proximidade excessiva da OLT sem a necessária perda de inserção óptica.".to_string(),
                            recommended_action: "Instalar atenuador óptico fixo de 5dB ou 10dB na CTO ou no conector de entrada do assinante para proteger o fotodiodo.".to_string(),
                            confidence_score: 95,
                            sample_onus,
                        });
                    }
                }

                // E) Diagnóstico: Falha Geral da Placa / Slot GPON Inteiro
                if total_pons_in_slot >= 3
                    && (slot_bad_pons as f64) / (total_pons_in_slot as f64) >= 0.60
                {
                    incidents.push(DiagnosticIncident {
                        id: format!("DIAG-SLOT-{}-{}", olt_name, slot_id),
                        severity: "critical".to_string(),
                        category: "slot_failure".to_string(),
                        title: format!("Alarme de Instabilidade Global na Placa GPON (Slot {})", slot_id),
                        olt_name: olt_name.clone(),
                        location: format!("Chassi OLT / Placa Slot {}", slot_id),
                        total_affected_onus: slot_bad_onus,
                        total_pon_onus: slot_total_onus,
                        avg_rx_dbm: -28.5,
                        root_cause: format!("{}/{} portas PON da placa Slot {} estão em estado crítico simultâneo. Causa provável: Falha na controladora de barramento da placa, superaquecimento do cartão ou oscilação de alimentação no sub-rack.", slot_bad_pons, total_pons_in_slot, slot_id),
                        recommended_action: "Verificar temperatura do chassi via gerência da OLT, inspecionar ventoinhas (Fan Tray) e testar reinicialização controlada da placa.".to_string(),
                        confidence_score: 94,
                        sample_onus: Vec::new(),
                    });
                }
            }
        }

        // F) Diagnóstico: Provisionamento Duplicado / ONT Fantasma (Universal Multi-Vendor)
        // Detecta quando o mesmo serial de ONU aparece cadastrado em múltiplas portas na mesma OLT
        let mut serial_location_map: HashMap<(&str, &str), Vec<&OnuRecord>> = HashMap::new();
        for onu in onus {
            let olt_name = onu.olt_name.as_deref().unwrap_or("OLT Principal");
            serial_location_map
                .entry((olt_name, onu.serial_number.as_str()))
                .or_default()
                .push(onu);
        }

        for ((olt_name, serial), instances) in serial_location_map {
            if instances.len() > 1 {
                // Separa entre instância ativa (Online com leitura) e inativa (Offline / Standby)
                let active_opt = instances
                    .iter()
                    .find(|i| i.status == "online" || i.latest_rx_power_dbm.is_some());
                let inactive_list: Vec<_> = instances
                    .iter()
                    .filter(|i| i.status != "online" && i.latest_rx_power_dbm.is_none())
                    .copied()
                    .collect();

                let active_loc = if let Some(a) = active_opt {
                    format!(
                        "Slot {} / Porta PON {} (ONU #{}) [Online: {:.2} dBm]",
                        a.slot,
                        a.pon_port,
                        a.onu_id,
                        a.latest_rx_power_dbm.unwrap_or(-20.0)
                    )
                } else {
                    let first = instances[0];
                    format!(
                        "Slot {} / Porta PON {} (ONU #{})",
                        first.slot, first.pon_port, first.onu_id
                    )
                };

                let inactive_locs: Vec<String> = if !inactive_list.is_empty() {
                    inactive_list
                        .iter()
                        .map(|i| {
                            format!(
                                "Slot {} / Porta PON {} (ONU #{})",
                                i.slot, i.pon_port, i.onu_id
                            )
                        })
                        .collect()
                } else {
                    instances
                        .iter()
                        .skip(1)
                        .map(|i| {
                            format!(
                                "Slot {} / Porta PON {} (ONU #{})",
                                i.slot, i.pon_port, i.onu_id
                            )
                        })
                        .collect()
                };

                let sample_onus = instances
                    .iter()
                    .map(|o| DiagnosticOnuSample {
                        serial_number: o.serial_number.clone(),
                        customer: o
                            .customer_identifier
                            .clone()
                            .or(o.custom_name.clone())
                            .unwrap_or_else(|| "--".to_string()),
                        slot: o.slot,
                        pon_port: o.pon_port,
                        rx_power_dbm: o.latest_rx_power_dbm,
                        delta_db: o.latest_delta_prev_rx_db,
                        status: o.status.clone(),
                    })
                    .collect();

                incidents.push(DiagnosticIncident {
                    id: format!("DIAG-DUP-{}-{}", olt_name, serial),
                    severity: "warning".to_string(),
                    category: "duplicate_provisioning".to_string(),
                    title: format!("Duplicidade de Provisionamento: {}", serial),
                    olt_name: olt_name.to_string(),
                    location: format!("{} vs {}", active_loc, inactive_locs.join(", ")),
                    total_affected_onus: instances.len(),
                    total_pon_onus: instances.len(),
                    avg_rx_dbm: active_opt.and_then(|a| a.latest_rx_power_dbm).unwrap_or(-20.0),
                    root_cause: format!(
                        "O equipamento físico com serial '{}' está cadastrado simultaneamente em múltiplas portas PON nesta OLT. Atualmente operacional em {} e com cadastro residual/inativo em: {}.",
                        serial, active_loc, inactive_locs.join("; ")
                    ),
                    recommended_action: format!(
                        "Acessar a gerência da OLT '{}' e desprovisionar/remover o cadastro residual da ONT em {} para liberar recursos de PON e eliminar duplicidade de configuração.",
                        olt_name, inactive_locs.join(", ")
                    ),
                    confidence_score: 99,
                    sample_onus,
                });
            }
        }

        // Ordena incidentes: Críticos primeiro, depois Warnings, depois Info
        incidents.sort_by(|a, b| {
            let rank = |s: &str| match s {
                "critical" => 1,
                "warning" => 2,
                _ => 3,
            };
            rank(&a.severity).cmp(&rank(&b.severity))
        });

        summary.total_incidents = incidents.len();
        summary.critical_incidents = incidents
            .iter()
            .filter(|i| i.severity == "critical")
            .count();
        summary.warning_incidents = incidents.iter().filter(|i| i.severity == "warning").count();
        summary.slot_incidents = incidents
            .iter()
            .filter(|i| i.category == "slot_failure")
            .count();
        summary.pon_incidents = incidents
            .iter()
            .filter(|i| i.category == "pon_sfp_issue")
            .count();
        summary.trunk_incidents = incidents
            .iter()
            .filter(|i| i.category == "trunk_degradation")
            .count();
        summary.incidents = incidents;

        summary
    }

    /// Executa avaliação de saúde de Upstream (Rx OLT na porta PON) e detecta assimetrias ópticas
    pub fn evaluate_upstream_diagnostics(onus: &[OnuRecord]) -> UpstreamDiagnosticsSummary {
        let mut summary = UpstreamDiagnosticsSummary::default();
        let mut items = Vec::new();
        let mut pon_map: HashMap<(String, i32, i32), Vec<&OnuRecord>> = HashMap::new();

        let mut audited_count = 0;

        for onu in onus {
            // Apenas ONUs ativas/online transmitem sinal óptico de Upstream para a OLT.
            // ONUs em LOS, Offline ou Dying Gasp não transmitem e devem ser descartadas da auditoria de Upstream.
            if onu.status != "online" {
                continue;
            }

            let olt_name = onu
                .olt_name
                .clone()
                .unwrap_or_else(|| "OLT Principal".to_string());
            let pon_key = (olt_name.clone(), onu.slot, onu.pon_port);
            pon_map.entry(pon_key).or_default().push(onu);

            // Considera para auditoria ONUs que possuem leitura de Rx na OLT dentro do envelope físico operacional
            if let Some(olt_rx) = onu.latest_olt_rx_power_dbm {
                if !(-45.0..=0.0).contains(&olt_rx) {
                    continue;
                }
                audited_count += 1;

                let rx_onu = onu.latest_rx_power_dbm;
                let tx_onu = onu.latest_tx_power_dbm;
                let olt_tx = onu.latest_olt_tx_power_dbm.unwrap_or(4.50);

                let down_att = rx_onu.map(|rx| olt_tx - rx);
                let up_att = tx_onu.map(|tx| tx - olt_rx);
                let asymmetry = match (down_att, up_att) {
                    (Some(d), Some(u)) => Some((d - u).abs()),
                    _ => None,
                };

                let is_sat_up = olt_rx > -12.0;
                let is_crit_up = olt_rx <= -27.0;
                let is_warn_up = olt_rx <= -25.0 && olt_rx > -27.0;
                let is_asymmetric = asymmetry.map(|a| a >= 3.0).unwrap_or(false);

                if is_sat_up || is_crit_up || is_warn_up || is_asymmetric {
                    let issue_type = if (is_crit_up || is_warn_up || is_sat_up) && is_asymmetric {
                        "both".to_string()
                    } else if is_sat_up {
                        "saturated_upstream".to_string()
                    } else if is_crit_up {
                        "critical_upstream".to_string()
                    } else if is_warn_up {
                        "warning_upstream".to_string()
                    } else {
                        "asymmetry".to_string()
                    };

                    let root_cause = if is_sat_up {
                        format!(
                            "Potência de recepção na OLT com risco de saturação ({:.2} dBm > -12.0 dBm). Causa provável: Drop óptico muito curto sem atenuador, provocando sobrecarga no fotodiodo APD da OLT e distorção de quadro.",
                            olt_rx
                        )
                    } else if is_asymmetric && is_crit_up {
                        if olt_rx <= -29.0 {
                            format!(
                                "Assimetria óptica severa ({:.2} dB) com Rx OLT crítico fora de especificação ({:.2} dBm < -29.0 dBm). Causa provável: Conector sujo/polimento imperfeito dispersando laser 1310nm, microcurvatura severa no drop ou laser da ONU esgotado.",
                                asymmetry.unwrap_or(0.0),
                                olt_rx
                            )
                        } else {
                            format!(
                                "Assimetria óptica severa ({:.2} dB) com Rx OLT marginal ({:.2} dBm entre -27 e -29 dBm). Causa provável: Refração seletiva em 1310nm ou conector SC-APC atenuado.",
                                asymmetry.unwrap_or(0.0),
                                olt_rx
                            )
                        }
                    } else if is_crit_up {
                        if olt_rx <= -29.0 {
                            format!(
                                "Potência de recepção na OLT crítica/fora de especificação ({:.2} dBm < -29.0 dBm). No limiar de sensibilidade do receptor APD (Classes B+/C+), com descarte de ACKs e risco iminente de perda de sincronismo (LOSi).",
                                olt_rx
                            )
                        } else {
                            format!(
                                "Potência de recepção na OLT marginal ({:.2} dBm entre -27 e -29 dBm). Margem de segurança exígua contra variações térmicas diárias e atenuação na fibra.",
                                olt_rx
                            )
                        }
                    } else if is_asymmetric {
                        format!(
                            "Diferença de atenuação de {:.2} dB entre Downstream e Upstream (Normal < 1.0 dB). Causa provável: Conector óptico com sujeira oleosa ou refração seletiva em 1310nm.",
                            asymmetry.unwrap_or(0.0)
                        )
                    } else {
                        format!(
                            "Potência de recepção na OLT em atenção ({:.2} dBm entre -25 e -27 dBm). Margem de segurança preventiva reduzida.",
                            olt_rx
                        )
                    };

                    let recommended_action = if is_sat_up {
                        "Instalar atenuador óptico (ex: 3 a 5 dB) no DIO ou PTO do assinante para reduzir a potência de recepção e proteger o APD da OLT.".to_string()
                    } else if is_crit_up || is_asymmetric {
                        "Inspecionar conectorização SC-APC da ONU na CTO/PTO, verificar potência de emissão Tx da ONU com Power Meter e medir atenuação da fibra.".to_string()
                    } else {
                        "Acompanhar telemetria preventiva de Upstream e inspecionar conectorização caso ocorram oscilações de upload.".to_string()
                    };

                    items.push(UpstreamIssueItem {
                        onu_id: onu.id,
                        olt_id: onu.olt_id,
                        olt_name,
                        slot: onu.slot,
                        pon_port: onu.pon_port,
                        onu_id_num: onu.onu_id,
                        serial_number: onu.serial_number.clone(),
                        customer_identifier: onu
                            .customer_identifier
                            .clone()
                            .or_else(|| onu.custom_name.clone()),
                        rx_power_dbm: rx_onu,
                        tx_power_dbm: tx_onu,
                        olt_rx_power_dbm: Some(olt_rx),
                        olt_tx_power_dbm: Some(olt_tx),
                        downstream_attenuation_db: down_att,
                        upstream_attenuation_db: up_att,
                        asymmetry_db: asymmetry,
                        issue_type,
                        root_cause,
                        recommended_action,
                    });
                }
            }
        }

        // Análise de Portas PON com anomalia de Upstream (Saturação, Crítica ou Atenção Coletiva)
        let mut suspect_pons = Vec::new();
        for ((olt_name, slot, pon_port), pon_onus) in pon_map {
            let mut pon_olt_rxs = Vec::new();
            let mut pon_sat_up = 0;
            let mut pon_crit_up = 0;
            let mut pon_warn_up = 0;

            for o in &pon_onus {
                if o.status != "online" {
                    continue;
                }
                if let Some(r) = o.latest_olt_rx_power_dbm {
                    if !(-45.0..=0.0).contains(&r) {
                        continue;
                    }
                    pon_olt_rxs.push(r);
                    if r > -12.0 {
                        pon_sat_up += 1;
                    } else if r <= -27.0 {
                        pon_crit_up += 1;
                    } else if r <= -25.0 {
                        pon_warn_up += 1;
                    }
                }
            }

            let total_with_rx = pon_olt_rxs.len();
            if total_with_rx > 0 {
                let avg_rx = pon_olt_rxs.iter().sum::<f64>() / (total_with_rx as f64);

                // A classificação da porta PON é estritamente determinada pela potência média de recepção óptica na porta (Rx na PON):
                // 1. Saturação na PON: Média > -12.0 dBm
                // 2. Crítica na PON: Média <= -27.0 dBm
                // 3. Atenção na PON: Média <= -25.0 dBm
                // Portas com média nominal/saudável (-12.0 a -25.0 dBm) não entram no painel de anomalias coletivas
                let (pon_status, is_suspect) = if avg_rx > -12.0 {
                    ("saturated".to_string(), true)
                } else if avg_rx <= -27.0 {
                    ("critical".to_string(), true)
                } else if avg_rx <= -25.0 {
                    ("warning".to_string(), false)
                } else {
                    ("healthy".to_string(), false)
                };

                if pon_status != "healthy" {
                    suspect_pons.push(PonUpstreamHealth {
                        olt_name,
                        slot,
                        pon_port,
                        total_onus: total_with_rx,
                        saturated_upstream_count: pon_sat_up,
                        critical_upstream_count: pon_crit_up,
                        warning_upstream_count: pon_warn_up,
                        avg_olt_rx_dbm: Some(avg_rx),
                        pon_status,
                        is_sfp_receiver_suspect: is_suspect,
                    });
                }
            }
        }

        // Ordena itens: "both" e "critical_upstream" primeiro, depois saturados, depois por assimetria decrescente
        items.sort_by(|a, b| {
            let rank = |it: &str| match it {
                "both" => 1,
                "critical_upstream" => 2,
                "saturated_upstream" => 3,
                "warning_upstream" => 4,
                "asymmetry" => 5,
                _ => 6,
            };
            let r_a = rank(&a.issue_type);
            let r_b = rank(&b.issue_type);
            if r_a != r_b {
                r_a.cmp(&r_b)
            } else {
                b.asymmetry_db
                    .unwrap_or(0.0)
                    .partial_cmp(&a.asymmetry_db.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        // Ordena suspect_pons por severidade ("saturated" -> "critical" -> "warning"), depois por quantidade de críticas decrescente
        suspect_pons.sort_by(|a, b| {
            let rank = |st: &str| match st {
                "saturated" => 1,
                "critical" => 2,
                "warning" => 3,
                _ => 4,
            };
            let r_a = rank(&a.pon_status);
            let r_b = rank(&b.pon_status);
            if r_a != r_b {
                r_a.cmp(&r_b)
            } else {
                b.critical_upstream_count
                    .cmp(&a.critical_upstream_count)
                    .then_with(|| b.warning_upstream_count.cmp(&a.warning_upstream_count))
            }
        });

        summary.total_audited_onus = audited_count;
        summary.saturated_upstream_count = items
            .iter()
            .filter(|i| {
                i.issue_type == "saturated_upstream"
                    || (i.issue_type == "both"
                        && i.olt_rx_power_dbm.map(|r| r > -12.0).unwrap_or(false))
            })
            .count();
        summary.critical_upstream_count = items
            .iter()
            .filter(|i| {
                i.issue_type == "critical_upstream"
                    || (i.issue_type == "both"
                        && i.olt_rx_power_dbm.map(|r| r <= -27.0).unwrap_or(false))
            })
            .count();
        summary.warning_upstream_count = items
            .iter()
            .filter(|i| {
                i.issue_type == "warning_upstream"
                    || (i.issue_type == "both"
                        && i.olt_rx_power_dbm
                            .map(|r| r <= -25.0 && r > -27.0)
                            .unwrap_or(false))
            })
            .count();
        summary.asymmetric_count = items
            .iter()
            .filter(|i| i.issue_type == "asymmetry" || i.issue_type == "both")
            .count();
        summary.suspect_pons_count = suspect_pons
            .iter()
            .filter(|p| p.is_sfp_receiver_suspect)
            .count();
        summary.items = items;
        summary.suspect_pons = suspect_pons;

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_evaluate_upstream_diagnostics_detects_critical_and_asymmetry() {
        let now = Utc::now().naive_utc();
        let onus = vec![
            OnuRecord {
                id: 1,
                olt_id: 10,
                olt_name: Some("OLT-CENTRAL".to_string()),
                slot: 1,
                pon_port: 1,
                onu_id: 1,
                serial_number: "ZTEGC1111111".to_string(),
                mac_address: None,
                model: Some("F670L".to_string()),
                custom_name: None,
                customer_identifier: Some("CLIENTE_CRITICO".to_string()),
                distance_meters: Some(2500),
                status: "online".to_string(),
                first_seen_at: now,
                last_seen_at: now,
                latest_rx_power_dbm: Some(-20.0), // Downstream normal
                latest_tx_power_dbm: Some(2.5),
                latest_olt_rx_power_dbm: Some(-28.5), // Upstream crítico!
                latest_olt_tx_power_dbm: Some(4.5),
                latest_attenuation_db: Some(24.5),
                latest_temperature_c: Some(42.0),
                latest_signal_quality: Some("good".to_string()),
                latest_delta_prev_rx_db: Some(0.0),
                is_degraded: Some(false),
            },
            OnuRecord {
                id: 2,
                olt_id: 10,
                olt_name: Some("OLT-CENTRAL".to_string()),
                slot: 1,
                pon_port: 1,
                onu_id: 2,
                serial_number: "ZTEGC2222222".to_string(),
                mac_address: None,
                model: Some("F670L".to_string()),
                custom_name: None,
                customer_identifier: Some("CLIENTE_NORMAL".to_string()),
                distance_meters: Some(1500),
                status: "online".to_string(),
                first_seen_at: now,
                last_seen_at: now,
                latest_rx_power_dbm: Some(-19.0),
                latest_tx_power_dbm: Some(2.5),
                latest_olt_rx_power_dbm: Some(-20.5), // Upstream excelente
                latest_olt_tx_power_dbm: Some(4.5),
                latest_attenuation_db: Some(23.5),
                latest_temperature_c: Some(40.0),
                latest_signal_quality: Some("good".to_string()),
                latest_delta_prev_rx_db: Some(0.0),
                is_degraded: Some(false),
            },
        ];

        let summary = OpticalEvaluator::evaluate_upstream_diagnostics(&onus);
        assert_eq!(summary.total_audited_onus, 2);
        assert_eq!(summary.critical_upstream_count, 1);
        assert_eq!(summary.items.len(), 1);
        assert_eq!(summary.items[0].serial_number, "ZTEGC1111111");
        assert!(
            summary.items[0].issue_type == "both"
                || summary.items[0].issue_type == "critical_upstream"
        );
    }

    #[test]
    fn test_evaluate_upstream_diagnostics_saturation_and_warning() {
        let now = Utc::now().naive_utc();
        let onus = vec![
            OnuRecord {
                id: 10,
                olt_id: 1,
                olt_name: Some("OLT-CENTRAL".to_string()),
                slot: 1,
                pon_port: 1,
                onu_id: 1,
                serial_number: "ZTEGCSAT1111".to_string(),
                mac_address: None,
                model: Some("F670L".to_string()),
                custom_name: None,
                customer_identifier: Some("CLIENTE_SATURADO".to_string()),
                distance_meters: Some(100),
                status: "online".to_string(),
                first_seen_at: now,
                last_seen_at: now,
                latest_rx_power_dbm: Some(-10.0),
                latest_tx_power_dbm: Some(2.5),
                latest_olt_rx_power_dbm: Some(-11.5), // Saturado (> -12.0)
                latest_olt_tx_power_dbm: Some(5.0),
                latest_attenuation_db: Some(15.0),
                latest_temperature_c: Some(40.0),
                latest_signal_quality: Some("excellent".to_string()),
                latest_delta_prev_rx_db: Some(0.0),
                is_degraded: Some(false),
            },
            OnuRecord {
                id: 11,
                olt_id: 1,
                olt_name: Some("OLT-CENTRAL".to_string()),
                slot: 1,
                pon_port: 1,
                onu_id: 2,
                serial_number: "ZTEGCWARN222".to_string(),
                mac_address: None,
                model: Some("F670L".to_string()),
                custom_name: None,
                customer_identifier: Some("CLIENTE_AVISO".to_string()),
                distance_meters: Some(3000),
                status: "online".to_string(),
                first_seen_at: now,
                last_seen_at: now,
                latest_rx_power_dbm: Some(-22.0),
                latest_tx_power_dbm: Some(2.5),
                latest_olt_rx_power_dbm: Some(-26.2), // Atenção (-25 a -27)
                latest_olt_tx_power_dbm: Some(5.0),
                latest_attenuation_db: Some(27.0),
                latest_temperature_c: Some(40.0),
                latest_signal_quality: Some("good".to_string()),
                latest_delta_prev_rx_db: Some(0.0),
                is_degraded: Some(false),
            },
        ];

        let summary = OpticalEvaluator::evaluate_upstream_diagnostics(&onus);
        assert_eq!(summary.total_audited_onus, 2);
        assert_eq!(summary.saturated_upstream_count, 1);
        assert_eq!(summary.warning_upstream_count, 1);
        assert_eq!(summary.critical_upstream_count, 0);
    }

    #[test]
    fn test_pon_saturation_only_when_pon_rx_exceeds_threshold() {
        let now = Utc::now().naive_utc();
        let mut onus = Vec::new();

        // PON 1 (Slot 1 / Port 1): 2 ONUs saturadas (-11.0 dBm), mas média geral -21.40 dBm (Saudável)
        // Não deve ser classificada como saturada nem entrar na lista de anomalias da PON
        for i in 1..=2 {
            onus.push(OnuRecord {
                id: i,
                olt_id: 1,
                olt_name: Some("OLT-CENTRAL".to_string()),
                slot: 1,
                pon_port: 1,
                onu_id: i as i32,
                serial_number: format!("ZTEGPON1SAT{}", i),
                mac_address: None,
                model: Some("F670L".to_string()),
                custom_name: None,
                customer_identifier: Some("CLIENTE_PROXIMO".to_string()),
                distance_meters: Some(50),
                status: "online".to_string(),
                first_seen_at: now,
                last_seen_at: now,
                latest_rx_power_dbm: Some(-10.0),
                latest_tx_power_dbm: Some(2.5),
                latest_olt_rx_power_dbm: Some(-11.0), // Saturada individual
                latest_olt_tx_power_dbm: Some(5.0),
                latest_attenuation_db: Some(15.0),
                latest_temperature_c: Some(40.0),
                latest_signal_quality: Some("excellent".to_string()),
                latest_delta_prev_rx_db: Some(0.0),
                is_degraded: Some(false),
            });
        }
        for i in 3..=10 {
            onus.push(OnuRecord {
                id: i,
                olt_id: 1,
                olt_name: Some("OLT-CENTRAL".to_string()),
                slot: 1,
                pon_port: 1,
                onu_id: i as i32,
                serial_number: format!("ZTEGPON1NORM{}", i),
                mac_address: None,
                model: Some("F670L".to_string()),
                custom_name: None,
                customer_identifier: Some("CLIENTE_NORMAL".to_string()),
                distance_meters: Some(1500),
                status: "online".to_string(),
                first_seen_at: now,
                last_seen_at: now,
                latest_rx_power_dbm: Some(-20.0),
                latest_tx_power_dbm: Some(2.5),
                latest_olt_rx_power_dbm: Some(-24.0), // Normal
                latest_olt_tx_power_dbm: Some(5.0),
                latest_attenuation_db: Some(25.0),
                latest_temperature_c: Some(40.0),
                latest_signal_quality: Some("good".to_string()),
                latest_delta_prev_rx_db: Some(0.0),
                is_degraded: Some(false),
            });
        }

        // PON 2 (Slot 1 / Port 2): Porta verdadeiramente Saturada (Média > -12.0 dBm)
        for i in 11..=14 {
            onus.push(OnuRecord {
                id: i,
                olt_id: 1,
                olt_name: Some("OLT-CENTRAL".to_string()),
                slot: 1,
                pon_port: 2,
                onu_id: (i - 10) as i32,
                serial_number: format!("ZTEGPON2SAT{}", i),
                mac_address: None,
                model: Some("F670L".to_string()),
                custom_name: None,
                customer_identifier: Some("CLIENTE_COLETIVO_SATURADO".to_string()),
                distance_meters: Some(40),
                status: "online".to_string(),
                first_seen_at: now,
                last_seen_at: now,
                latest_rx_power_dbm: Some(-9.0),
                latest_tx_power_dbm: Some(2.5),
                latest_olt_rx_power_dbm: Some(-10.5), // Média > -12.0 dBm
                latest_olt_tx_power_dbm: Some(5.0),
                latest_attenuation_db: Some(14.0),
                latest_temperature_c: Some(40.0),
                latest_signal_quality: Some("excellent".to_string()),
                latest_delta_prev_rx_db: Some(0.0),
                is_degraded: Some(false),
            });
        }

        // PON 3 (Slot 1 / Port 3): Porta Crítica (Média <= -27.0 dBm)
        for i in 15..=17 {
            onus.push(OnuRecord {
                id: i,
                olt_id: 1,
                olt_name: Some("OLT-CENTRAL".to_string()),
                slot: 1,
                pon_port: 3,
                onu_id: (i - 14) as i32,
                serial_number: format!("ZTEGPON3CRIT{}", i),
                mac_address: None,
                model: Some("F670L".to_string()),
                custom_name: None,
                customer_identifier: Some("CLIENTE_CRITICO".to_string()),
                distance_meters: Some(4000),
                status: "online".to_string(),
                first_seen_at: now,
                last_seen_at: now,
                latest_rx_power_dbm: Some(-27.0),
                latest_tx_power_dbm: Some(2.5),
                latest_olt_rx_power_dbm: Some(-28.0), // Média <= -27.0 dBm
                latest_olt_tx_power_dbm: Some(5.0),
                latest_attenuation_db: Some(32.0),
                latest_temperature_c: Some(40.0),
                latest_signal_quality: Some("critical".to_string()),
                latest_delta_prev_rx_db: Some(0.0),
                is_degraded: Some(false),
            });
        }

        let summary = OpticalEvaluator::evaluate_upstream_diagnostics(&onus);

        // A PON 1 (Slot 1 / Port 1) tem média -21.4 dBm (Saudável). NÃO deve estar em suspect_pons!
        assert!(!summary
            .suspect_pons
            .iter()
            .any(|p| p.slot == 1 && p.pon_port == 1));

        // A PON 2 (Slot 1 / Port 2) tem média -10.5 dBm (> -12.0 dBm). DEVE estar em suspect_pons como "saturated"!
        let pon2 = summary
            .suspect_pons
            .iter()
            .find(|p| p.slot == 1 && p.pon_port == 2);
        assert!(pon2.is_some());
        assert_eq!(pon2.unwrap().pon_status, "saturated");
        assert!(pon2.unwrap().is_sfp_receiver_suspect);

        // A PON 3 (Slot 1 / Port 3) tem média -28.0 dBm (<= -27.0 dBm). DEVE estar em suspect_pons como "critical"!
        let pon3 = summary
            .suspect_pons
            .iter()
            .find(|p| p.slot == 1 && p.pon_port == 3);
        assert!(pon3.is_some());
        assert_eq!(pon3.unwrap().pon_status, "critical");
        assert!(pon3.unwrap().is_sfp_receiver_suspect);

        // Apenas PON 2 e PON 3 devem constar na lista de suspect_pons
        assert_eq!(summary.suspect_pons.len(), 2);
    }

    #[test]
    fn test_evaluate_upstream_diagnostics_ignores_los_and_offline_onus() {
        let now = Utc::now().naive_utc();
        let onus = vec![
            // ONU 1: Em LOS com resíduo de -3.91 dBm (caso real reportado)
            OnuRecord {
                id: 101,
                olt_id: 1,
                olt_name: Some("AMA-LOJA-OLT01".to_string()),
                slot: 15,
                pon_port: 16,
                onu_id: 39,
                serial_number: "ZTEGDA60070D".to_string(),
                mac_address: None,
                model: Some("F670L".to_string()),
                custom_name: None,
                customer_identifier: Some("60070d".to_string()),
                distance_meters: Some(1200),
                status: "los".to_string(), // Em LOS!
                first_seen_at: now,
                last_seen_at: now,
                latest_rx_power_dbm: Some(-24.09),
                latest_tx_power_dbm: Some(1.65),
                latest_olt_rx_power_dbm: Some(-3.91), // Leitura residual / apócrifa enquanto offline
                latest_olt_tx_power_dbm: Some(5.0),
                latest_attenuation_db: Some(29.0),
                latest_temperature_c: Some(40.0),
                latest_signal_quality: Some("offline".to_string()),
                latest_delta_prev_rx_db: Some(0.0),
                is_degraded: Some(false),
            },
            // ONU 2: Em Dying Gasp com resíduo de -3.56 dBm
            OnuRecord {
                id: 102,
                olt_id: 1,
                olt_name: Some("AMA-LOJA-OLT01".to_string()),
                slot: 15,
                pon_port: 16,
                onu_id: 33,
                serial_number: "ZTEGDA6060A2".to_string(),
                mac_address: None,
                model: Some("F670L".to_string()),
                custom_name: None,
                customer_identifier: Some("onuztegda6060a2".to_string()),
                distance_meters: Some(800),
                status: "dying_gasp".to_string(), // Desligada
                first_seen_at: now,
                last_seen_at: now,
                latest_rx_power_dbm: Some(-23.87),
                latest_tx_power_dbm: Some(3.05),
                latest_olt_rx_power_dbm: Some(-3.56),
                latest_olt_tx_power_dbm: Some(5.0),
                latest_attenuation_db: Some(28.0),
                latest_temperature_c: Some(38.0),
                latest_signal_quality: Some("offline".to_string()),
                latest_delta_prev_rx_db: Some(0.0),
                is_degraded: Some(false),
            },
            // ONU 3: Online com sinal perfeitamente saudável (-22.50 dBm)
            OnuRecord {
                id: 103,
                olt_id: 1,
                olt_name: Some("AMA-LOJA-OLT01".to_string()),
                slot: 15,
                pon_port: 16,
                onu_id: 1,
                serial_number: "ZTEGDA111111".to_string(),
                mac_address: None,
                model: Some("F670L".to_string()),
                custom_name: None,
                customer_identifier: Some("CLIENTE_ONLINE".to_string()),
                distance_meters: Some(1500),
                status: "online".to_string(), // Online!
                first_seen_at: now,
                last_seen_at: now,
                latest_rx_power_dbm: Some(-20.0),
                latest_tx_power_dbm: Some(2.5),
                latest_olt_rx_power_dbm: Some(-22.50), // Normal / Saudável
                latest_olt_tx_power_dbm: Some(5.0),
                latest_attenuation_db: Some(25.0),
                latest_temperature_c: Some(40.0),
                latest_signal_quality: Some("good".to_string()),
                latest_delta_prev_rx_db: Some(0.0),
                is_degraded: Some(false),
            },
        ];

        let summary = OpticalEvaluator::evaluate_upstream_diagnostics(&onus);

        // Apenas a ONU 3 (online) deve ser auditada
        assert_eq!(summary.total_audited_onus, 1);
        // Nenhuma ONU em LOS/Dying Gasp pode ser computada como saturada ou crítica
        assert_eq!(summary.saturated_upstream_count, 0);
        assert_eq!(summary.critical_upstream_count, 0);
        assert_eq!(summary.warning_upstream_count, 0);
        assert_eq!(summary.items.len(), 0);
        // A porta tem apenas 1 ONU online com -22.50 dBm (Saudável), portanto não deve ser suspeita
        assert_eq!(summary.suspect_pons.len(), 0);
    }
}
