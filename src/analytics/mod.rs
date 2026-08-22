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

        if rx >= -18.0 && rx <= -14.0 {
            SignalClassification::Excellent
        } else if rx >= -23.0 && rx < -18.0 {
            SignalClassification::Good
        } else if rx >= -26.99 && rx < -23.0 {
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
                let delta = curr - prev;
                // is_degraded quando a perda de potência óptica for maior ou igual ao threshold configurado (ex: delta <= -3.0 dB)
                let is_degraded = delta <= -cfg.degradation_alert_delta_db.abs();
                (Some(delta), is_degraded)
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
}
