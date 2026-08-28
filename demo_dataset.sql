-- ==============================================================================
-- SignalHunter: Dataset Demo Multi-Vendor (Incluindo TP-Link DeltaStream GPON)
-- Popula OLTs, ONUs e Séries Temporais de Telemetria Óptica DDM para Demonstração
-- ==============================================================================

SET FOREIGN_KEY_CHECKS = 0;

-- 1. OLT TP-Link DeltaStream DS-P7001-16
INSERT INTO olts (id, name, ip_address, vendor, model, firmware_version, snmp_port, snmp_community_encrypted, is_active, collection_interval_mins, max_concurrent_requests, pon_delay_ms, last_collected_at, last_collection_status, created_at)
VALUES 
(10, 'OLT-TPLINK-CENTRAL-01', '10.200.70.10', 'tplink', 'DeltaStream DS-P7001-16', '1.2.0_Build_20251225', 161, NULL, 1, 15, 2, 50, NOW(), 'success', NOW() - INTERVAL 30 DAY)
ON DUPLICATE KEY UPDATE 
    name = VALUES(name),
    vendor = VALUES(vendor),
    model = VALUES(model),
    firmware_version = VALUES(firmware_version),
    last_collected_at = VALUES(last_collected_at),
    last_collection_status = VALUES(last_collection_status);

-- 2. ONUs TP-Link (DeltaStream GPON / XZ000-G3 / XZ000-G7 / Archer XR500v)
INSERT INTO onus (id, olt_id, slot, pon_port, onu_id, serial_number, mac_address, model, custom_name, customer_identifier, distance_meters, status, first_seen_at, last_seen_at)
VALUES
(1001, 10, 1, 1, 1, 'TPLG9A4B1001', 'B0:95:75:A4:B1:01', 'TP-Link XZ000-G3', 'Assinante 1001', 'CLI-TPLINK-1001', 1250, 'online', NOW() - INTERVAL 30 DAY, NOW()),
(1002, 10, 1, 1, 2, 'TPLG9A4B1002', 'B0:95:75:A4:B1:02', 'TP-Link XZ000-G7', 'Assinante 1002', 'CLI-TPLINK-1002', 1840, 'online', NOW() - INTERVAL 30 DAY, NOW()),
(1003, 10, 1, 1, 3, 'TPLG9A4B1003', 'B0:95:75:A4:B1:03', 'TP-Link Archer XR500v', 'Assinante 1003 (Crítico)', 'CLI-TPLINK-1003', 3420, 'online', NOW() - INTERVAL 30 DAY, NOW()),
(1004, 10, 1, 1, 4, 'TPLG9A4B1004', 'B0:95:75:A4:B1:04', 'TP-Link XZ000-G3', 'Assinante 1004 (Alerta Degradação)', 'CLI-TPLINK-1004', 2110, 'online', NOW() - INTERVAL 30 DAY, NOW()),
(1005, 10, 1, 2, 1, 'TPLG9A4B1005', 'B0:95:75:A4:B1:05', 'TP-Link XC220-G3v', 'Assinante 1005', 'CLI-TPLINK-1005', 980, 'online', NOW() - INTERVAL 30 DAY, NOW()),
(1006, 10, 1, 2, 2, 'TPLG9A4B1006', 'B0:95:75:A4:B1:06', 'TP-Link XZ000-G3', 'Assinante 1006 (Atenuado)', 'CLI-TPLINK-1006', 4150, 'online', NOW() - INTERVAL 30 DAY, NOW()),
(1007, 10, 1, 3, 1, 'TPLG9A4B1007', 'B0:95:75:A4:B1:07', 'TP-Link Archer XR500v', 'Assinante 1007 (Offline LOS)', 'CLI-TPLINK-1007', 2800, 'los', NOW() - INTERVAL 30 DAY, NOW() - INTERVAL 2 HOUR),
(1008, 10, 1, 3, 2, 'TPLG9A4B1008', 'B0:95:75:A4:B1:08', 'TP-Link XZ000-G7', 'Assinante 1008 (Dying Gasp)', 'CLI-TPLINK-1008', 1530, 'dying_gasp', NOW() - INTERVAL 30 DAY, NOW() - INTERVAL 1 HOUR)
ON DUPLICATE KEY UPDATE
    olt_id = VALUES(olt_id),
    slot = VALUES(slot),
    pon_port = VALUES(pon_port),
    onu_id = VALUES(onu_id),
    model = VALUES(model),
    custom_name = VALUES(custom_name),
    customer_identifier = VALUES(customer_identifier),
    distance_meters = VALUES(distance_meters),
    status = VALUES(status),
    last_seen_at = VALUES(last_seen_at);

-- 3. Histórico de Séries Temporais Ópticas (Últimas 5 Coletas por ONU)

-- ONU 1001: Sinal Excelente (-16.5 dBm estável)
INSERT INTO onu_signal_history (onu_id, collected_at, rx_power_dbm, tx_power_dbm, olt_rx_power_dbm, olt_tx_power_dbm, attenuation_db, voltage_v, bias_current_ma, temperature_c, signal_quality, delta_prev_rx_db, is_degraded, collection_protocol)
VALUES
(1001, NOW() - INTERVAL 4 HOUR, -16.45, 2.30, -19.20, 2.50, 18.95, 3.31, 14.50, 42.1, 'excellent', 0.00, 0, 'snmp'),
(1001, NOW() - INTERVAL 3 HOUR, -16.48, 2.32, -19.25, 2.50, 18.98, 3.30, 14.60, 42.5, 'excellent', -0.03, 0, 'snmp'),
(1001, NOW() - INTERVAL 2 HOUR, -16.50, 2.30, -19.22, 2.50, 19.00, 3.31, 14.50, 43.0, 'excellent', -0.02, 0, 'snmp'),
(1001, NOW() - INTERVAL 1 HOUR, -16.52, 2.31, -19.30, 2.50, 19.02, 3.30, 14.70, 43.2, 'excellent', -0.02, 0, 'snmp'),
(1001, NOW(),                 -16.50, 2.30, -19.25, 2.50, 19.00, 3.31, 14.50, 42.8, 'excellent',  0.02, 0, 'snmp');

-- ONU 1002: Sinal Bom (-20.10 dBm)
INSERT INTO onu_signal_history (onu_id, collected_at, rx_power_dbm, tx_power_dbm, olt_rx_power_dbm, olt_tx_power_dbm, attenuation_db, voltage_v, bias_current_ma, temperature_c, signal_quality, delta_prev_rx_db, is_degraded, collection_protocol)
VALUES
(1002, NOW() - INTERVAL 4 HOUR, -19.90, 2.45, -22.10, 2.50, 22.40, 3.29, 16.20, 45.0, 'good', 0.00, 0, 'snmp'),
(1002, NOW() - INTERVAL 3 HOUR, -19.95, 2.40, -22.15, 2.50, 22.45, 3.30, 16.10, 45.2, 'good', -0.05, 0, 'snmp'),
(1002, NOW() - INTERVAL 2 HOUR, -20.02, 2.42, -22.20, 2.50, 22.52, 3.28, 16.30, 46.1, 'good', -0.07, 0, 'snmp'),
(1002, NOW() - INTERVAL 1 HOUR, -20.08, 2.40, -22.25, 2.50, 22.58, 3.29, 16.20, 45.8, 'good', -0.06, 0, 'snmp'),
(1002, NOW(),                 -20.12, 2.41, -22.30, 2.50, 22.62, 3.29, 16.40, 45.5, 'good', -0.04, 0, 'snmp');

-- ONU 1003: Sinal Crítico (-28.40 dBm - Rompimento Parcial / Conector Sujo)
INSERT INTO onu_signal_history (onu_id, collected_at, rx_power_dbm, tx_power_dbm, olt_rx_power_dbm, olt_tx_power_dbm, attenuation_db, voltage_v, bias_current_ma, temperature_c, signal_quality, delta_prev_rx_db, is_degraded, collection_protocol)
VALUES
(1003, NOW() - INTERVAL 4 HOUR, -27.80, 2.10, -29.90, 2.50, 30.30, 3.25, 22.50, 51.2, 'critical', 0.00, 0, 'snmp'),
(1003, NOW() - INTERVAL 3 HOUR, -28.00, 2.12, -30.10, 2.50, 30.50, 3.24, 22.80, 52.0, 'critical', -0.20, 0, 'snmp'),
(1003, NOW() - INTERVAL 2 HOUR, -28.15, 2.10, -30.25, 2.50, 30.65, 3.25, 23.10, 52.5, 'critical', -0.15, 0, 'snmp'),
(1003, NOW() - INTERVAL 1 HOUR, -28.30, 2.08, -30.40, 2.50, 30.80, 3.23, 23.40, 53.1, 'critical', -0.15, 0, 'snmp'),
(1003, NOW(),                 -28.45, 2.05, -30.60, 2.50, 30.95, 3.22, 23.90, 53.8, 'critical', -0.15, 1, 'snmp');

-- ONU 1004: Degradação Acentuada (Delta-dB = 4.30 dB de perda recente - Macrocurvatura na Caixa de Emenda)
INSERT INTO onu_signal_history (onu_id, collected_at, rx_power_dbm, tx_power_dbm, olt_rx_power_dbm, olt_tx_power_dbm, attenuation_db, voltage_v, bias_current_ma, temperature_c, signal_quality, delta_prev_rx_db, is_degraded, collection_protocol)
VALUES
(1004, NOW() - INTERVAL 4 HOUR, -19.20, 2.50, -21.40, 2.50, 21.70, 3.30, 15.00, 41.0, 'good',     0.00, 0, 'snmp'),
(1004, NOW() - INTERVAL 3 HOUR, -19.25, 2.48, -21.45, 2.50, 21.75, 3.31, 15.10, 41.2, 'good',    -0.05, 0, 'snmp'),
(1004, NOW() - INTERVAL 2 HOUR, -21.50, 2.45, -23.80, 2.50, 24.00, 3.29, 16.50, 43.0, 'good',    -2.25, 0, 'snmp'),
(1004, NOW() - INTERVAL 1 HOUR, -23.10, 2.40, -25.50, 2.50, 25.60, 3.28, 17.80, 44.5, 'warning', -1.60, 1, 'snmp'),
(1004, NOW(),                 -23.50, 2.38, -26.00, 2.50, 26.00, 3.27, 18.20, 45.0, 'warning', -0.40, 1, 'snmp');

-- ONU 1005: Sinal Normal (-18.20 dBm)
INSERT INTO onu_signal_history (onu_id, collected_at, rx_power_dbm, tx_power_dbm, olt_rx_power_dbm, olt_tx_power_dbm, attenuation_db, voltage_v, bias_current_ma, temperature_c, signal_quality, delta_prev_rx_db, is_degraded, collection_protocol)
VALUES
(1005, NOW() - INTERVAL 4 HOUR, -18.10, 2.35, -20.30, 2.50, 20.60, 3.32, 14.80, 40.5, 'good', 0.00, 0, 'snmp'),
(1005, NOW() - INTERVAL 3 HOUR, -18.15, 2.32, -20.35, 2.50, 20.65, 3.31, 14.90, 40.8, 'good', -0.05, 0, 'snmp'),
(1005, NOW() - INTERVAL 2 HOUR, -18.18, 2.35, -20.40, 2.50, 20.68, 3.30, 14.80, 41.2, 'good', -0.03, 0, 'snmp'),
(1005, NOW() - INTERVAL 1 HOUR, -18.22, 2.30, -20.45, 2.50, 20.72, 3.32, 15.00, 41.5, 'good', -0.04, 0, 'snmp'),
(1005, NOW(),                 -18.20, 2.34, -20.40, 2.50, 20.70, 3.31, 14.90, 41.0, 'good',  0.02, 0, 'snmp');

-- ONU 1006: Sinal em Alerta de Atenuação (-25.80 dBm)
INSERT INTO onu_signal_history (onu_id, collected_at, rx_power_dbm, tx_power_dbm, olt_rx_power_dbm, olt_tx_power_dbm, attenuation_db, voltage_v, bias_current_ma, temperature_c, signal_quality, delta_prev_rx_db, is_degraded, collection_protocol)
VALUES
(1006, NOW() - INTERVAL 4 HOUR, -25.50, 2.20, -27.80, 2.50, 28.00, 3.26, 19.50, 48.0, 'warning', 0.00, 0, 'snmp'),
(1006, NOW() - INTERVAL 3 HOUR, -25.60, 2.18, -27.90, 2.50, 28.10, 3.25, 19.70, 48.2, 'warning', -0.10, 0, 'snmp'),
(1006, NOW() - INTERVAL 2 HOUR, -25.72, 2.20, -28.05, 2.50, 28.22, 3.26, 19.90, 48.9, 'warning', -0.12, 0, 'snmp'),
(1006, NOW() - INTERVAL 1 HOUR, -25.80, 2.15, -28.15, 2.50, 28.30, 3.24, 20.10, 49.3, 'warning', -0.08, 0, 'snmp'),
(1006, NOW(),                 -25.85, 2.16, -28.20, 2.50, 28.35, 3.25, 20.30, 49.0, 'warning', -0.05, 0, 'snmp');

-- 4. Registro de Auditoria da Coleta TP-Link
INSERT INTO audit_logs (user_id, action, resource_type, resource_id, details, ip_address, created_at)
VALUES
(1, 'COLLECT_SNMP', 'OLT', '10', 'Varredura 100% SNMPv2c concluída com sucesso na OLT-TPLINK-CENTRAL-01 (8 ONUs analisadas, 1 crítica, 2 alertas, 2 offline)', '127.0.0.1', NOW() - INTERVAL 2 MINUTE);

SET FOREIGN_KEY_CHECKS = 1;
