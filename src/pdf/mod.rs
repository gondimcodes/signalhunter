use crate::db::queries::OnuRecord;
use anyhow::{Context, Result};
use chrono::Local;
use printpdf::*;
use std::path::Path;

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN: f32 = 15.0;

// Font sizes (points)
const TITLE_SIZE: f32 = 15.0;
const SUBTITLE_SIZE: f32 = 8.5;
const SECTION_SIZE: f32 = 8.0;
const TH_SIZE: f32 = 8.0;
const TD_SIZE: f32 = 7.5;
const FOOTER_SIZE: f32 = 7.0;

// Line heights (mm)
const ROW_H: f32 = 5.2;

// Table column positions (mm from left page edge)
const COL_POS: f32 = MARGIN + 1.0;
const COL_SERIAL: f32 = MARGIN + 8.5;
const COL_CLIENT: f32 = MARGIN + 36.0;
const COL_PON: f32 = MARGIN + 68.0;
const COL_RX: f32 = MARGIN + 89.0;
const COL_HISTORY: f32 = MARGIN + 109.0;
const COL_STATUS: f32 = MARGIN + 167.0;

struct PdfWriter {
    doc: PdfDocument,
    page_index: usize,
    y: f32,
    logo_image: Option<RawImage>,
}

impl PdfWriter {
    fn new(title: &str) -> Result<Self> {
        let mut doc = PdfDocument::new(title);
        doc.pages.push(PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), vec![]));

        // Carrega a logo exata do ampscan desofuscada em memória para o relatório PDF
        let logo_bytes = crate::assets::get_embedded_pdf_logo();
        let mut warnings = Vec::new();
        let logo_image = RawImage::decode_from_bytes(logo_bytes, &mut warnings).ok();

        Ok(Self {
            doc,
            page_index: 0,
            y: PAGE_H - MARGIN,
            logo_image,
        })
    }

    fn new_page(&mut self) {
        self.doc
            .pages
            .push(PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), vec![]));
        self.page_index = self.doc.pages.len() - 1;
        self.y = PAGE_H - MARGIN;
    }

    fn ensure_space(&mut self, needed_mm: f32) {
        if self.y - needed_mm < (MARGIN + 12.0) {
            self.new_page();
        }
    }

    fn text_at(&mut self, text: &str, size: f32, x: f32, bold: bool) {
        let font = if bold {
            PdfFontHandle::Builtin(BuiltinFont::HelveticaBold)
        } else {
            PdfFontHandle::Builtin(BuiltinFont::Helvetica)
        };
        let page = &mut self.doc.pages[self.page_index];
        page.ops.push(Op::StartTextSection);
        page.ops.push(Op::SetTextCursor {
            pos: Point::new(Mm(x), Mm(self.y)),
        });
        page.ops.push(Op::SetFont {
            font,
            size: Pt(size),
        });
        page.ops.push(Op::ShowText {
            items: vec![TextItem::Text(sanitize(text))],
        });
        page.ops.push(Op::EndTextSection);
    }

    fn hline(&mut self, y: f32, start_x: f32, end_x: f32) {
        let page = &mut self.doc.pages[self.page_index];
        let line = Line {
            points: vec![
                LinePoint {
                    p: Point::new(Mm(start_x), Mm(y)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(end_x), Mm(y)),
                    bezier: false,
                },
            ],
            is_closed: false,
        };
        page.ops.push(Op::DrawLine { line });
    }

    fn draw_header_bar(&mut self, now_str: &str, operator_name: &str, report_subtitle: &str) {
        if let Some(ref raw_img) = self.logo_image.clone() {
            let scale = 0.50; // Exatamente a escala configurada no ampscan
            let logo_height = scale * 30.0; // Cálculo dinâmico de altura do ampscan (15mm)
            self.ensure_space(logo_height + 5.0);

            let image_id = self.doc.add_image(raw_img);
            let page = &mut self.doc.pages[self.page_index];
            page.ops.push(Op::UseXobject {
                id: image_id,
                transform: XObjectTransform {
                    translate_x: Some(Mm(MARGIN).into()),
                    translate_y: Some(Mm(self.y - logo_height).into()),
                    scale_x: Some(scale),
                    scale_y: Some(scale),
                    ..Default::default()
                },
            });
            self.y -= logo_height + 5.0;
        } else {
            self.y -= 10.0;
        }

        self.text_at("SignalHunter", TITLE_SIZE, MARGIN, true);
        self.y -= 5.5;
        self.text_at(report_subtitle, 11.0, MARGIN, false);
        self.y -= 5.0;
        self.text_at(
            &format!(
                "Emissao: {}  |  Operador: {}  |  Ambiente: Producao NOC",
                now_str, operator_name
            ),
            SUBTITLE_SIZE,
            MARGIN,
            false,
        );
        self.y -= 4.0;
        self.hline(self.y, MARGIN, PAGE_W - MARGIN);
        self.y -= 5.0;
    }

    fn draw_table_header(&mut self, is_degradation: bool) {
        self.ensure_space(ROW_H * 2.0);
        self.text_at("Pos", TH_SIZE, COL_POS, true);
        self.text_at("Serial ONU", TH_SIZE, COL_SERIAL, true);
        self.text_at("Cliente / Identificador", TH_SIZE, COL_CLIENT, true);
        self.text_at("Porta PON", TH_SIZE, COL_PON, true);
        if is_degradation {
            self.text_at("Sinal Anterior", TH_SIZE, MARGIN + 106.0, true);
            self.text_at("Sinal Atual", TH_SIZE, MARGIN + 125.0, true);
            self.text_at("Piora (Delta-dB)", TH_SIZE, MARGIN + 144.0, true);
            self.text_at("Status", TH_SIZE, MARGIN + 167.0, true);
        } else {
            self.text_at("Sinal Rx", TH_SIZE, COL_RX, true);
            self.text_at("Ultimas 5 Leituras (Historico)", TH_SIZE, COL_HISTORY, true);
            self.text_at("Status", TH_SIZE, COL_STATUS, true);
        }
        self.y -= 1.8;
        self.hline(self.y, MARGIN, PAGE_W - MARGIN);
        self.y -= 3.8;
    }

    fn save(self, path: &str) -> Result<()> {
        let mut warnings = Vec::new();
        let bytes = self.doc.save(&PdfSaveOptions::default(), &mut warnings);
        std::fs::write(path, bytes)
            .with_context(|| format!("Failed to create PDF file: {}", path))?;
        Ok(())
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'è' | 'ê' => 'e',
            'í' | 'ì' | 'î' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'û' => 'u',
            'ç' => 'c',
            'Á' | 'À' | 'Â' | 'Ã' => 'A',
            'É' | 'È' | 'Ê' => 'E',
            'Í' | 'Ì' | 'Î' => 'I',
            'Ó' | 'Ò' | 'Ô' | 'Õ' => 'O',
            'Ú' | 'Ù' | 'Û' => 'U',
            'Ç' => 'C',
            _ => c,
        })
        .collect()
}

pub struct PdfReportGenerator;

impl PdfReportGenerator {
    /// Gera um relatório técnico analítico em PDF corporativo de alta precisão visual
    /// com logo ISPFocus no topo e histórico das últimas 5 coletas por ONU
    pub fn generate_optical_report<P: AsRef<Path>>(
        output_path: P,
        scope_title: &str,
        operator_name: &str,
        critical_onus: &[OnuRecord],
        history_map: &std::collections::HashMap<u64, Vec<f64>>,
        olt_info_map: &std::collections::HashMap<String, (String, Option<String>, i64, i64)>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut w = PdfWriter::new("SignalHunter - Optical Audit Report")?;
        let now_str = Local::now().format("%d/%m/%Y %H:%M:%S").to_string();
        let is_degradation_report = scope_title.to_lowercase().contains("piora")
            || scope_title.to_lowercase().contains("delta");

        // Agrupa as ONUs por OLT
        let mut olt_groups: std::collections::BTreeMap<String, Vec<&OnuRecord>> =
            std::collections::BTreeMap::new();
        for onu in critical_onus {
            let olt_name = onu
                .olt_name
                .clone()
                .unwrap_or_else(|| "OLT Nao Identificada".to_string());
            olt_groups.entry(olt_name).or_default().push(onu);
        }

        // Ordenação por OLT estritamente na seguinte ordem:
        // 1. Críticas (sinal baixo < -27.0 dBm)
        // 2. Atenção (sinal entre -23.0 dBm e -27.0 dBm ou degradação)
        // 3. LOS (Rompimento / Perda de sinal óptico)
        // 4. Dying Gasp (Falta de energia / Desligadas)
        // 5. Saturadas (sinal excessivo > -14.0 dBm - logo após Dying Gasp)
        // 6. Normais (sinal excelente/bom entre -14.0 dBm e -23.0 dBm)
        for list in olt_groups.values_mut() {
            list.sort_by(|a, b| {
                let priority = |onu: &OnuRecord| -> (i32, f64, String) {
                    let rx = onu.latest_rx_power_dbm;
                    let is_offline = rx.is_none()
                        || onu.status == "offline"
                        || onu.status == "los"
                        || onu.status == "dying_gasp";
                    let is_dying_gasp = onu.status == "dying_gasp";
                    let is_los = is_offline && !is_dying_gasp;

                    let tier = if !is_offline {
                        let rx_val = rx.unwrap_or(0.0);
                        if rx_val > -14.0 {
                            5 // 5. Saturadas (logo após Dying Gasp)
                        } else if rx_val < -27.0 {
                            1 // 1. Críticas (sinal baixo)
                        } else if rx_val < -23.0 || onu.is_degraded == Some(true) {
                            2 // 2. Atenção
                        } else {
                            6 // 6. Normais
                        }
                    } else if is_los {
                        3 // 3. LOS
                    } else {
                        4 // 4. Dying Gasp
                    };

                    (tier, rx.unwrap_or(0.0), onu.serial_number.clone())
                };

                let p_a = priority(a);
                let p_b = priority(b);

                p_a.0
                    .cmp(&p_b.0)
                    .then_with(|| {
                        p_a.1
                            .partial_cmp(&p_b.1)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then_with(|| p_a.2.cmp(&p_b.2))
            });
        }

        w.draw_header_bar(&now_str, operator_name, scope_title);

        if olt_groups.is_empty() {
            w.text_at(
                "Nenhum alerta de sinal critico ou atencao registrado no momento.",
                10.0,
                MARGIN,
                false,
            );
            w.save(output_path.as_ref().to_str().unwrap_or("report.pdf"))?;
            return Ok(());
        }

        for (olt_name, onus) in &olt_groups {
            w.ensure_space(18.0);
            w.y -= 2.0;

            let (vendor, model_opt, total_onus, total_alerts) =
                olt_info_map.get(olt_name).cloned().unwrap_or_else(|| {
                    (
                        "ZTE".to_string(),
                        None,
                        onus.len() as i64,
                        onus.len() as i64,
                    )
                });

            let model_str = model_opt.as_deref().unwrap_or("--");

            let section_header = if is_degradation_report {
                format!(
                    "OLT: {}  |  Marca: {}  |  Modelo: {}  |  Total de ONUs: {}  |  Piora de Sinal (Delta-dB): {}",
                    olt_name, vendor, model_str, total_onus, onus.len()
                )
            } else {
                format!(
                        "OLT: {}  |  Marca: {}  |  Modelo: {}  |  Total de ONUs: {}  |  Total de Alertas: {}",
                        olt_name, vendor, model_str, total_onus, total_alerts
                    )
            };

            w.text_at(&section_header, SECTION_SIZE, MARGIN, true);
            w.y -= 4.5;
            w.draw_table_header(is_degradation_report);

            for (idx, onu) in onus.iter().enumerate() {
                w.ensure_space(ROW_H);

                let rx_str = onu
                    .latest_rx_power_dbm
                    .map(|v| format!("{:.2} dBm", v))
                    .unwrap_or_else(|| "--".to_string());
                let pon_str = format!("S{}/P{}:{}", onu.slot, onu.pon_port, onu.onu_id);
                let client_name = onu
                    .customer_identifier
                    .as_deref()
                    .or(onu.custom_name.as_deref())
                    .unwrap_or("--");
                let client_short = if client_name.len() > 18 {
                    format!("{}...", &client_name[..16])
                } else {
                    client_name.to_string()
                };

                let rx_opt = onu.latest_rx_power_dbm;
                let is_offline = rx_opt.is_none()
                    || onu.status == "offline"
                    || onu.status == "los"
                    || onu.status == "dying_gasp";

                let (status_lbl, is_bold_status) = if is_offline {
                    if onu.status == "dying_gasp" {
                        ("DYING GASP", false)
                    } else {
                        ("LOS (ROMP)", true)
                    }
                } else {
                    let rx_val = rx_opt.unwrap();
                    if rx_val > -14.0 {
                        ("SATURADO", true)
                    } else if rx_val < -27.0 {
                        ("CRITICO", true)
                    } else if rx_val < -23.0 || (onu.is_degraded == Some(true) && rx_val < -22.0) {
                        ("ATENCAO", false)
                    } else {
                        ("NORMAL", false)
                    }
                };

                w.text_at(&format!("{}", idx + 1), TD_SIZE, COL_POS, false);
                w.text_at(&onu.serial_number, TD_SIZE, COL_SERIAL, true);
                w.text_at(&client_short, TD_SIZE, COL_CLIENT, false);
                w.text_at(&pon_str, TD_SIZE, COL_PON, false);

                if is_degradation_report {
                    let delta_val = onu.latest_delta_prev_rx_db.unwrap_or(0.0);
                    // Cálculo do sinal anterior: rx_anterior = rx_atual + delta (se delta foi perda)
                    let prev_rx_str = rx_opt
                        .map(|curr| format!("{:.2} dBm", curr - delta_val))
                        .unwrap_or_else(|| "--".to_string());
                    let delta_str = format!("{:.2} dB", delta_val);

                    w.text_at(&prev_rx_str, TD_SIZE, MARGIN + 106.0, false);
                    w.text_at(&rx_str, TD_SIZE, MARGIN + 125.0, true);
                    w.text_at(&delta_str, TD_SIZE, MARGIN + 144.0, true);
                    w.text_at(status_lbl, TD_SIZE, MARGIN + 167.0, is_bold_status);
                } else {
                    w.text_at(&rx_str, TD_SIZE, COL_RX, true);

                    // Formata as últimas leituras de sinal com 2 casas decimais (ex: "-21.25  -21.43  -21.49  -21.43  -21.37")
                    let hist_str = if let Some(vals) = history_map.get(&onu.id) {
                        if vals.is_empty() {
                            "--".to_string()
                        } else {
                            vals.iter()
                                .take(5)
                                .map(|v| format!("{:.2}", v))
                                .collect::<Vec<String>>()
                                .join("  ")
                        }
                    } else if let Some(curr_rx) = rx_opt {
                        format!("{:.2}", curr_rx)
                    } else {
                        "--".to_string()
                    };

                    w.text_at(&hist_str, TD_SIZE - 0.7, COL_HISTORY, false);
                    w.text_at(status_lbl, TD_SIZE, COL_STATUS, is_bold_status);
                }

                w.y -= ROW_H;
            }

            w.y -= 4.0;
        }

        // Rodapé nas páginas
        let total_p = w.doc.pages.len();
        for (i, page) in w.doc.pages.iter_mut().enumerate() {
            page.ops.push(Op::StartTextSection);
            page.ops.push(Op::SetTextCursor {
                pos: Point::new(Mm(MARGIN), Mm(8.0)),
            });
            page.ops.push(Op::SetFont {
                font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                size: Pt(FOOTER_SIZE),
            });
            page.ops.push(Op::ShowText {
                items: vec![TextItem::Text(format!(
                    "SignalHunter Intelligent PON Diagnostics  -  Documento Confidencial de Engenharia/NOC  |  Pagina {} de {}",
                    i + 1,
                    total_p
                ))],
            });
            page.ops.push(Op::EndTextSection);
        }

        w.save(output_path.as_ref().to_str().unwrap_or("report.pdf"))?;
        Ok(())
    }

    /// Gera o Laudo Pericial de Diagnóstico Óptico e Análise de Causa Raiz (RCA)
    pub fn generate_diagnostics_report<P: AsRef<Path>>(
        output_path: P,
        operator_name: &str,
        summary: &crate::analytics::DiagnosticSummary,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut w = PdfWriter::new("SignalHunter - Optical Root Cause Analysis")?;
        let now_str = Local::now().format("%d/%m/%Y %H:%M:%S").to_string();

        w.draw_header_bar(
            &now_str,
            operator_name,
            "Laudo Técnico de Diagnóstico Óptico & Causa Raiz (RCA)",
        );

        // Resumo Executivo
        w.ensure_space(20.0);
        let exec_summary = format!(
            "Sintese Geral: {} Incidente(s) Detectado(s) | Criticos: {} | Atencao: {} | Falhas SFP PON: {} | Tronco/Rota: {}",
            summary.total_incidents,
            summary.critical_incidents,
            summary.warning_incidents,
            summary.pon_incidents,
            summary.trunk_incidents
        );
        w.text_at(&exec_summary, 9.0, MARGIN, true);
        w.y -= 4.0;
        w.hline(self_y(&w), MARGIN, PAGE_W - MARGIN);
        w.y -= 6.0;

        if summary.incidents.is_empty() {
            w.text_at(
                "Nenhuma anomalia topológica ou falha de porta/slot detectada na rede óptica.",
                10.0,
                MARGIN,
                false,
            );
            w.save(output_path.as_ref().to_str().unwrap_or("report.pdf"))?;
            return Ok(());
        }

        for (idx, inc) in summary.incidents.iter().enumerate() {
            w.ensure_space(34.0);

            let is_crit = inc.severity == "critical";
            let badge_str = if is_crit {
                "[FALHA CRITICA]"
            } else if inc.severity == "warning" {
                "[ATENCAO]"
            } else {
                "[INFORMATIVO]"
            };

            // Título do Incidente
            let title_line = format!(
                "{}. {} {} - Precisao do Laudo: {}%",
                idx + 1,
                badge_str,
                sanitize(&inc.title),
                inc.confidence_score
            );
            w.text_at(&title_line, 9.5, MARGIN, true);
            w.y -= 4.5;

            // Localização e Impacto
            let loc_line = format!(
                "Equipamento: {} | Localizacao: {} | Impacto: {} de {} ONUs afetadas",
                sanitize(&inc.olt_name),
                sanitize(&inc.location),
                inc.total_affected_onus,
                inc.total_pon_onus
            );
            w.text_at(&loc_line, 8.0, MARGIN + 2.0, false);
            w.y -= 4.2;

            // Causa Provável (Root Cause)
            let cause_text = format!("Causa Provavel: {}", sanitize(&inc.root_cause));
            let lines_cause = wrap_text(&cause_text, 105);
            for l in lines_cause {
                w.ensure_space(ROW_H);
                w.text_at(&l, 7.8, MARGIN + 2.0, false);
                w.y -= 3.6;
            }

            // Ação Recomendada
            let action_text = format!("Acao Recomendada: {}", sanitize(&inc.recommended_action));
            let lines_action = wrap_text(&action_text, 105);
            for (i, l) in lines_action.iter().enumerate() {
                w.ensure_space(ROW_H);
                w.text_at(l, 7.8, MARGIN + 2.0, i == 0);
                w.y -= 3.6;
            }

            // Amostra de ONUs afetadas
            if !inc.sample_onus.is_empty() {
                let mut sample_parts = Vec::new();
                for s in inc.sample_onus.iter().take(4) {
                    let rx_val = s
                        .rx_power_dbm
                        .map(|v| format!("{:.2}dBm", v))
                        .unwrap_or_else(|| "Offline".to_string());
                    sample_parts.push(format!("{}({})", s.serial_number, rx_val));
                }
                let sample_line = format!("Amostra ONUs: {}", sample_parts.join(", "));
                w.text_at(&sample_line, 7.2, MARGIN + 2.0, false);
                w.y -= 3.8;
            }

            w.y -= 2.0;
            w.hline(self_y(&w), MARGIN, PAGE_W - MARGIN);
            w.y -= 5.0;
        }

        // Rodapé nas páginas
        let total_p = w.doc.pages.len();
        for (i, page) in w.doc.pages.iter_mut().enumerate() {
            page.ops.push(Op::StartTextSection);
            page.ops.push(Op::SetTextCursor {
                pos: Point::new(Mm(MARGIN), Mm(8.0)),
            });
            page.ops.push(Op::SetFont {
                font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                size: Pt(FOOTER_SIZE),
            });
            page.ops.push(Op::ShowText {
                items: vec![TextItem::Text(format!(
                    "SignalHunter Intelligent Optical Diagnostics (RCA)  -  Documento Confidencial NOC  |  Pagina {} de {}",
                    i + 1,
                    total_p
                ))],
            });
            page.ops.push(Op::EndTextSection);
        }

        w.save(output_path.as_ref().to_str().unwrap_or("report.pdf"))?;
        Ok(())
    }

    /// Gera o relatório técnico de Inventário de Modelos e Firmwares das OLTs
    pub fn generate_olt_firmware_report<P: AsRef<Path>>(
        output_path: P,
        operator_name: &str,
        olts: &[crate::handlers::report_handlers::OltFirmwareItem],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut w = PdfWriter::new("SignalHunter - Inventario de Modelos e Firmwares OLT")?;
        let now_str = Local::now().format("%d/%m/%Y %H:%M:%S").to_string();

        w.draw_header_bar(
            &now_str,
            operator_name,
            "Inventario de Hardware, Modelos e Firmwares das OLTs",
        );

        // Subtítulo descritivo
        w.ensure_space(ROW_H * 3.0);
        let summary_text = format!(
            "Total de Equipamentos Mapeados: {} OLTs  |  Status Geral: {} Ativas / {} Inativas",
            olts.len(),
            olts.iter().filter(|o| o.is_active).count(),
            olts.iter().filter(|o| !o.is_active).count()
        );
        w.text_at(&summary_text, SECTION_SIZE, MARGIN + 1.0, true);
        w.y -= 4.0;
        w.hline(w.y, MARGIN, PAGE_W - MARGIN);
        w.y -= 4.0;

        // Cabeçalho da Tabela
        let col_pos = MARGIN + 1.0;
        let col_ip = MARGIN + 7.0;
        let col_host = MARGIN + 35.0;
        let col_vendor = MARGIN + 68.0;
        let col_model = MARGIN + 90.0;
        let col_fw = MARGIN + 138.0;
        let col_status = MARGIN + 172.0;

        let draw_table_header_fw = |writer: &mut PdfWriter| {
            writer.ensure_space(ROW_H * 2.0);
            writer.text_at("#", TH_SIZE, col_pos, true);
            writer.text_at("Endereco IP", TH_SIZE, col_ip, true);
            writer.text_at("Hostname", TH_SIZE, col_host, true);
            writer.text_at("Marca", TH_SIZE, col_vendor, true);
            writer.text_at("Modelo OLT", TH_SIZE, col_model, true);
            writer.text_at("Versao Firmware", TH_SIZE, col_fw, true);
            writer.text_at("Status", TH_SIZE, col_status, true);
            writer.y -= 1.8;
            writer.hline(writer.y, MARGIN, PAGE_W - MARGIN);
            writer.y -= 3.8;
        };

        draw_table_header_fw(&mut w);

        for (idx, item) in olts.iter().enumerate() {
            if w.y < MARGIN + 18.0 {
                w.new_page();
                w.draw_header_bar(
                    &now_str,
                    operator_name,
                    "Inventario de Hardware, Modelos e Firmwares das OLTs (Cont.)",
                );
                draw_table_header_fw(&mut w);
            }

            let clean_host = sanitize(&item.hostname);
            let host_display =
                if clean_host.is_empty() || clean_host == "N/D" || clean_host == "Inacessivel" {
                    "--"
                } else {
                    &clean_host
                };

            let clean_vendor = sanitize(&item.vendor.to_uppercase());
            let vendor_display = if clean_vendor.is_empty() || clean_vendor == "N/D" {
                "--"
            } else {
                &clean_vendor
            };

            let clean_model = sanitize(&item.model);
            let model_display = if clean_model.is_empty() || clean_model == "N/D" {
                "--"
            } else {
                &clean_model
            };

            let clean_fw = sanitize(&item.firmware_version);
            let fw_display = if clean_fw.is_empty()
                || clean_fw == "N/D"
                || clean_fw == "Timeout / Inacessivel"
            {
                "--"
            } else {
                &clean_fw
            };

            w.text_at(&(idx + 1).to_string(), TD_SIZE, col_pos, false);
            w.text_at(&item.ip_address, TD_SIZE, col_ip, true);
            w.text_at(host_display, TD_SIZE, col_host, false);
            w.text_at(vendor_display, TD_SIZE, col_vendor, false);
            w.text_at(model_display, TD_SIZE, col_model, true);
            w.text_at(fw_display, TD_SIZE, col_fw, false);

            let status_label = if !item.is_active {
                "Desativada"
            } else if item.is_online {
                "Online"
            } else {
                "Offline"
            };
            w.text_at(status_label, TD_SIZE, col_status, true);

            w.y -= ROW_H;
        }

        // Rodapé
        let total_p = w.doc.pages.len();
        for (i, page) in w.doc.pages.iter_mut().enumerate() {
            page.ops.push(Op::StartTextSection);
            page.ops.push(Op::SetTextCursor {
                pos: Point::new(Mm(MARGIN), Mm(8.0)),
            });
            page.ops.push(Op::SetFont {
                font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                size: Pt(FOOTER_SIZE),
            });
            page.ops.push(Op::ShowText {
                items: vec![TextItem::Text(format!(
                    "SignalHunter OLT Hardware & Firmware Inventory  -  Documento Confidencial NOC  |  Pagina {} de {}",
                    i + 1,
                    total_p
                ))],
            });
            page.ops.push(Op::EndTextSection);
        }

        w.save(output_path.as_ref().to_str().unwrap_or("report.pdf"))?;
        Ok(())
    }
}

fn self_y(w: &PdfWriter) -> f32 {
    w.y
}

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut current_line = String::new();

    for word in words {
        if current_line.len() + word.len() + 1 > max_chars {
            if !current_line.is_empty() {
                lines.push(current_line.clone());
                current_line.clear();
            }
        }
        if !current_line.is_empty() {
            current_line.push(' ');
        }
        current_line.push_str(word);
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    lines
}
