-- ==============================================================================
-- SignalHunter: Dataset Demo Completo Multi-Vendor Homologado
-- Popula OLTs, ONUs e Séries Temporais de Telemetria Óptica DDM
-- Fabricantes: ZTE, Huawei, Datacom, FiberHome, Nokia, Parks e TP-Link
-- ==============================================================================

SET FOREIGN_KEY_CHECKS = 0;

-- -----------------------------------------------------------------------------
-- 1. OLTs DE DEMONSTRAÇÃO (TODOS OS FABRICANTES)
-- -----------------------------------------------------------------------------
INSERT INTO olts (id, name, ip_address, vendor, model, firmware_version, snmp_port, snmp_community_encrypted, is_active, collection_interval_mins, max_concurrent_requests, pon_delay_ms, last_collected_at, last_collection_status, created_at)
VALUES 
(1, 'OLT-ZTE-TITAN-01',    '10.200.10.1', 'zte',       'ZTE C600 Titan',         'V1.2.5P3',            161, NULL, 1, 15, 2, 50, NOW(), 'success', NOW() - INTERVAL 60 DAY),
(2, 'OLT-HUAWEI-MA5800-01','10.200.20.1', 'huawei',    'SmartAX MA5800-X17',     'V800R018C10',         161, NULL, 1, 15, 2, 50, NOW(), 'success', NOW() - INTERVAL 60 DAY),
(3, 'OLT-DATACOM-DM4610-01','10.200.30.1', 'datacom',   'DmOS DM4610 16GPON',     '12.6.4',              161, NULL, 1, 15, 2, 50, NOW(), 'success', NOW() - INTERVAL 60 DAY),
(4, 'OLT-FIBERHOME-5516-01','10.200.40.1', 'fiberhome', 'AN5516-01 GPON',        'RP1000',              161, NULL, 1, 15, 2, 50, NOW(), 'success', NOW() - INTERVAL 60 DAY),
(5, 'OLT-NOKIA-ISAM-01',   '10.200.50.1', 'nokia',     'ISAM 7360 FX-16',        'R6.2.04',             161, NULL, 1, 15, 2, 50, NOW(), 'success', NOW() - INTERVAL 60 DAY),
(6, 'OLT-PARKS-FIBERLINK-01','10.200.60.1','parks',     'Fiberlink 21016',        '2.1.4',               161, NULL, 1, 15, 2, 50, NOW(), 'success', NOW() - INTERVAL 60 DAY),
(10,'OLT-TPLINK-CENTRAL-01','10.200.70.10','tplink',    'DeltaStream DS-P7001-16', '1.2.0_Build_20251225',161, NULL, 1, 15, 2, 50, NOW(), 'success', NOW() - INTERVAL 30 DAY)
ON DUPLICATE KEY UPDATE 
    name = VALUES(name),
    vendor = VALUES(vendor),
    model = VALUES(model),
    firmware_version = VALUES(firmware_version),
    last_collected_at = VALUES(last_collected_at),
    last_collection_status = VALUES(last_collection_status);

-- -----------------------------------------------------------------------------
-- 2. ONUs MULTI-VENDOR
-- -----------------------------------------------------------------------------
INSERT INTO onus (id, olt_id, slot, pon_port, onu_id, serial_number, mac_address, model, custom_name, customer_identifier, distance_meters, status, first_seen_at, last_seen_at)
VALUES
-- ZTE (OLT 1)
(101, 1, 1, 1, 1, 'ZTEGC1010001', '00:1A:C2:01:00:01', 'ZXHN F670L',   'Cliente ZTE 01', 'CLI-ZTE-101', 1100, 'online', NOW() - INTERVAL 60 DAY, NOW()),
(102, 1, 1, 1, 2, 'ZTEGC1010002', '00:1A:C2:01:00:02', 'ZXHN F601',    'Cliente ZTE 02 (Degradação)', 'CLI-ZTE-102', 2250, 'online', NOW() - INTERVAL 60 DAY, NOW()),
(103, 1, 1, 2, 1, 'ZTEGC1010003', '00:1A:C2:01:00:03', 'ZXHN F6600P',  'Cliente ZTE 03 (Crítico)', 'CLI-ZTE-103', 3800, 'online', NOW() - INTERVAL 60 DAY, NOW()),
(104, 1, 1, 2, 2, 'ZTEGC1010004', '00:1A:C2:01:00:04', 'ZXHN F670L',   'Cliente ZTE 04 (LOS)', 'CLI-ZTE-104', 1900, 'los', NOW() - INTERVAL 60 DAY, NOW() - INTERVAL 1 HOUR),

-- Huawei (OLT 2)
(201, 2, 1, 1, 1, 'HWTC20100001', '48:57:02:01:00:01', 'OptiXstar EG8145V5', 'Cliente HW 01', 'CLI-HW-201', 950,  'online', NOW() - INTERVAL 60 DAY, NOW()),
(202, 2, 1, 1, 2, 'HWTC20100002', '48:57:02:01:00:02', 'EchoLife HG8010H',  'Cliente HW 02 (Alerta)', 'CLI-HW-202', 3100, 'online', NOW() - INTERVAL 60 DAY, NOW()),
(203, 2, 1, 2, 1, 'HWTC20100003', '48:57:02:01:00:03', 'OptiXstar HG8145X6', 'Cliente HW 03 (Dying Gasp)', 'CLI-HW-203', 1400, 'dying_gasp', NOW() - INTERVAL 60 DAY, NOW() - INTERVAL 30 MINUTE),

-- Datacom (OLT 3)
(301, 3, 1, 1, 1, 'DATA30100001', '00:04:DF:01:00:01', 'DM985-100',    'Cliente Datacom 01', 'CLI-DT-301', 1300, 'online', NOW() - INTERVAL 60 DAY, NOW()),
(302, 3, 1, 1, 2, 'DATA30100002', '00:04:DF:01:00:02', 'DM986-414',    'Cliente Datacom 02 (Piora)', 'CLI-DT-302', 2900, 'online', NOW() - INTERVAL 60 DAY, NOW()),

-- FiberHome (OLT 4)
(401, 4, 1, 1, 1, 'FHTT40100001', '00:0A:EB:01:00:01', 'HG6245D',      'Cliente FH 01', 'CLI-FH-401', 1200, 'online', NOW() - INTERVAL 60 DAY, NOW()),
(402, 4, 1, 1, 2, 'FHTT40100002', '00:0A:EB:01:00:02', 'AN5506-01-A',  'Cliente FH 02 (Crítico)', 'CLI-FH-402', 4200, 'online', NOW() - INTERVAL 60 DAY, NOW()),

-- Nokia (OLT 5)
(501, 5, 1, 1, 1, 'ALCL50100001', '00:20:D0:01:00:01', 'G-140W-ME',    'Cliente Nokia 01', 'CLI-NK-501', 850,  'online', NOW() - INTERVAL 60 DAY, NOW()),
(502, 5, 1, 1, 2, 'ALCL50100002', '00:20:D0:01:00:02', 'G-010G-R',     'Cliente Nokia 02', 'CLI-NK-502', 1750, 'online', NOW() - INTERVAL 60 DAY, NOW()),

-- Parks (OLT 6)
(601, 6, 1, 1, 1, 'PRKS60100001', '00:01:E8:01:00:01', 'Fiberlink 210', 'Cliente Parks 01', 'CLI-PK-601', 1150, 'online', NOW() - INTERVAL 60 DAY, NOW()),
(602, 6, 1, 1, 2, 'PRKS60100002', '00:01:E8:01:00:02', 'Fiberlink 611', 'Cliente Parks 02 (Degradação)', 'CLI-PK-602', 2600, 'online', NOW() - INTERVAL 60 DAY, NOW()),

-- TP-Link (OLT 10)
(1001, 10, 1, 1, 1, 'TPLG9A4B1001', 'B0:95:75:A4:B1:01', 'TP-Link XZ000-G3',     'Assinante TP-Link 01', 'CLI-TPLINK-1001', 1250, 'online', NOW() - INTERVAL 30 DAY, NOW()),
(1002, 10, 1, 1, 2, 'TPLG9A4B1002', 'B0:95:75:A4:B1:02', 'TP-Link XZ000-G7',     'Assinante TP-Link 02', 'CLI-TPLINK-1002', 1840, 'online', NOW() - INTERVAL 30 DAY, NOW()),
(1003, 10, 1, 1, 3, 'TPLG9A4B1003', 'B0:95:75:A4:B1:03', 'TP-Link Archer XR500v', 'Assinante TP-Link 03 (Crítico)', 'CLI-TPLINK-1003', 3420, 'online', NOW() - INTERVAL 30 DAY, NOW()),
(1004, 10, 1, 1, 4, 'TPLG9A4B1004', 'B0:95:75:A4:B1:04', 'TP-Link XZ000-G3',     'Assinante TP-Link 04 (Alerta Degradação)', 'CLI-TPLINK-1004', 2110, 'online', NOW() - INTERVAL 30 DAY, NOW()),
(1005, 10, 1, 2, 1, 'TPLG9A4B1005', 'B0:95:75:A4:B1:05', 'TP-Link XC220-G3v',    'Assinante TP-Link 05', 'CLI-TPLINK-1005', 980,  'online', NOW() - INTERVAL 30 DAY, NOW()),
(1006, 10, 1, 2, 2, 'TPLG9A4B1006', 'B0:95:75:A4:B1:06', 'TP-Link XZ000-G3',     'Assinante TP-Link 06 (Atenuado)', 'CLI-TPLINK-1006', 4150, 'online', NOW() - INTERVAL 30 DAY, NOW()),
(1007, 10, 1, 3, 1, 'TPLG9A4B1007', 'B0:95:75:A4:B1:07', 'TP-Link Archer XR500v', 'Assinante TP-Link 07 (Offline LOS)', 'CLI-TPLINK-1007', 2800, 'los', NOW() - INTERVAL 30 DAY, NOW() - INTERVAL 2 HOUR),
(1008, 10, 1, 3, 2, 'TPLG9A4B1008', 'B0:95:75:A4:B1:08', 'TP-Link XZ000-G7',     'Assinante TP-Link 08 (Dying Gasp)', 'CLI-TPLINK-1008', 1530, 'dying_gasp', NOW() - INTERVAL 30 DAY, NOW() - INTERVAL 1 HOUR)
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

-- -----------------------------------------------------------------------------
-- 3. SÉRIES TEMPORAIS ÓPTICAS (ÚLTIMAS 5 LEITURAS POR ONU)
-- -----------------------------------------------------------------------------

-- ZTE
INSERT INTO onu_signal_history (onu_id, collected_at, rx_power_dbm, tx_power_dbm, olt_rx_power_dbm, olt_tx_power_dbm, attenuation_db, voltage_v, bias_current_ma, temperature_c, signal_quality, delta_prev_rx_db, is_degraded, collection_protocol)
VALUES
(101, NOW() - INTERVAL 4 HOUR, -17.20, 2.40, -19.50, 2.50, 19.70, 3.30, 14.2, 41.5, 'excellent', 0.00, 0, 'snmp'),
(101, NOW() - INTERVAL 3 HOUR, -17.25, 2.41, -19.55, 2.50, 19.75, 3.31, 14.3, 41.8, 'excellent', -0.05, 0, 'snmp'),
(101, NOW() - INTERVAL 2 HOUR, -17.20, 2.40, -19.50, 2.50, 19.70, 3.30, 14.2, 42.0, 'excellent', 0.05, 0, 'snmp'),
(101, NOW() - INTERVAL 1 HOUR, -17.22, 2.39, -19.52, 2.50, 19.72, 3.30, 14.1, 41.9, 'excellent', -0.02, 0, 'snmp'),
(101, NOW(),                 -17.20, 2.40, -19.50, 2.50, 19.70, 3.30, 14.2, 42.1, 'excellent', 0.02, 0, 'snmp'),

(102, NOW() - INTERVAL 4 HOUR, -19.10, 2.50, -21.40, 2.50, 21.60, 3.30, 15.0, 43.0, 'good', 0.00, 0, 'snmp'),
(102, NOW() - INTERVAL 3 HOUR, -19.15, 2.48, -21.45, 2.50, 21.65, 3.30, 15.2, 43.2, 'good', -0.05, 0, 'snmp'),
(102, NOW() - INTERVAL 2 HOUR, -21.80, 2.45, -24.10, 2.50, 24.30, 3.28, 16.5, 45.0, 'good', -2.65, 0, 'snmp'),
(102, NOW() - INTERVAL 1 HOUR, -23.90, 2.40, -26.20, 2.50, 26.40, 3.27, 18.0, 46.5, 'warning', -2.10, 1, 'snmp'),
(102, NOW(),                 -24.30, 2.38, -26.60, 2.50, 26.80, 3.26, 18.5, 47.0, 'warning', -0.40, 1, 'snmp'),

(103, NOW() - INTERVAL 4 HOUR, -27.50, 2.10, -29.80, 2.50, 30.00, 3.24, 21.0, 50.0, 'critical', 0.00, 0, 'snmp'),
(103, NOW() - INTERVAL 3 HOUR, -27.60, 2.08, -29.90, 2.50, 30.10, 3.24, 21.2, 50.5, 'critical', -0.10, 0, 'snmp'),
(103, NOW() - INTERVAL 2 HOUR, -27.80, 2.05, -30.10, 2.50, 30.30, 3.23, 21.5, 51.0, 'critical', -0.20, 0, 'snmp'),
(103, NOW() - INTERVAL 1 HOUR, -28.00, 2.02, -30.30, 2.50, 30.50, 3.22, 22.0, 51.8, 'critical', -0.20, 0, 'snmp'),
(103, NOW(),                 -28.15, 2.00, -30.45, 2.50, 30.65, 3.22, 22.3, 52.0, 'critical', -0.15, 0, 'snmp');

-- Huawei
INSERT INTO onu_signal_history (onu_id, collected_at, rx_power_dbm, tx_power_dbm, olt_rx_power_dbm, olt_tx_power_dbm, attenuation_db, voltage_v, bias_current_ma, temperature_c, signal_quality, delta_prev_rx_db, is_degraded, collection_protocol)
VALUES
(201, NOW() - INTERVAL 4 HOUR, -16.80, 2.50, -19.10, 2.50, 19.30, 3.32, 13.8, 39.5, 'excellent', 0.00, 0, 'snmp'),
(201, NOW() - INTERVAL 3 HOUR, -16.82, 2.51, -19.12, 2.50, 19.32, 3.32, 13.9, 39.8, 'excellent', -0.02, 0, 'snmp'),
(201, NOW() - INTERVAL 2 HOUR, -16.80, 2.50, -19.10, 2.50, 19.30, 3.32, 13.8, 40.0, 'excellent', 0.02, 0, 'snmp'),
(201, NOW() - INTERVAL 1 HOUR, -16.85, 2.49, -19.15, 2.50, 19.35, 3.31, 14.0, 40.2, 'excellent', -0.05, 0, 'snmp'),
(201, NOW(),                 -16.80, 2.50, -19.10, 2.50, 19.30, 3.32, 13.8, 39.9, 'excellent', 0.05, 0, 'snmp'),

(202, NOW() - INTERVAL 4 HOUR, -24.80, 2.20, -27.10, 2.50, 27.30, 3.26, 18.9, 47.5, 'warning', 0.00, 0, 'snmp'),
(202, NOW() - INTERVAL 3 HOUR, -24.85, 2.18, -27.15, 2.50, 27.35, 3.25, 19.0, 47.8, 'warning', -0.05, 0, 'snmp'),
(202, NOW() - INTERVAL 2 HOUR, -24.90, 2.19, -27.20, 2.50, 27.40, 3.26, 19.2, 48.0, 'warning', -0.05, 0, 'snmp'),
(202, NOW() - INTERVAL 1 HOUR, -25.05, 2.15, -27.35, 2.50, 27.55, 3.25, 19.5, 48.5, 'warning', -0.15, 0, 'snmp'),
(202, NOW(),                 -25.10, 2.16, -27.40, 2.50, 27.60, 3.25, 19.6, 48.2, 'warning', -0.05, 0, 'snmp');

-- Datacom
INSERT INTO onu_signal_history (onu_id, collected_at, rx_power_dbm, tx_power_dbm, olt_rx_power_dbm, olt_tx_power_dbm, attenuation_db, voltage_v, bias_current_ma, temperature_c, signal_quality, delta_prev_rx_db, is_degraded, collection_protocol)
VALUES
(301, NOW() - INTERVAL 4 HOUR, -18.40, 2.30, -20.60, 2.50, 20.90, 3.30, 14.5, 42.0, 'good', 0.00, 0, 'snmp'),
(301, NOW() - INTERVAL 3 HOUR, -18.45, 2.32, -20.65, 2.50, 20.95, 3.31, 14.6, 42.2, 'good', -0.05, 0, 'snmp'),
(301, NOW() - INTERVAL 2 HOUR, -18.40, 2.30, -20.60, 2.50, 20.90, 3.30, 14.5, 42.5, 'good', 0.05, 0, 'snmp'),
(301, NOW() - INTERVAL 1 HOUR, -18.48, 2.31, -20.68, 2.50, 20.98, 3.30, 14.7, 42.8, 'good', -0.08, 0, 'snmp'),
(301, NOW(),                 -18.42, 2.30, -20.62, 2.50, 20.92, 3.31, 14.5, 42.3, 'good', 0.06, 0, 'snmp'),

(302, NOW() - INTERVAL 4 HOUR, -19.50, 2.40, -21.70, 2.50, 22.00, 3.30, 15.0, 44.0, 'good', 0.00, 0, 'snmp'),
(302, NOW() - INTERVAL 3 HOUR, -19.60, 2.38, -21.80, 2.50, 22.10, 3.29, 15.2, 44.3, 'good', -0.10, 0, 'snmp'),
(302, NOW() - INTERVAL 2 HOUR, -22.50, 2.35, -24.70, 2.50, 25.00, 3.28, 17.0, 46.0, 'warning', -2.90, 1, 'snmp'),
(302, NOW() - INTERVAL 1 HOUR, -23.80, 2.30, -26.00, 2.50, 26.30, 3.27, 18.2, 47.2, 'warning', -1.30, 1, 'snmp'),
(302, NOW(),                 -24.10, 2.28, -26.30, 2.50, 26.60, 3.26, 18.8, 47.5, 'warning', -0.30, 1, 'snmp');

-- FiberHome
INSERT INTO onu_signal_history (onu_id, collected_at, rx_power_dbm, tx_power_dbm, olt_rx_power_dbm, olt_tx_power_dbm, attenuation_db, voltage_v, bias_current_ma, temperature_c, signal_quality, delta_prev_rx_db, is_degraded, collection_protocol)
VALUES
(401, NOW() - INTERVAL 4 HOUR, -17.90, 2.35, -20.10, 2.50, 20.40, 3.31, 14.0, 40.0, 'good', 0.00, 0, 'snmp'),
(401, NOW() - INTERVAL 3 HOUR, -17.92, 2.34, -20.12, 2.50, 20.42, 3.31, 14.1, 40.2, 'good', -0.02, 0, 'snmp'),
(401, NOW() - INTERVAL 2 HOUR, -17.90, 2.35, -20.10, 2.50, 20.40, 3.31, 14.0, 40.5, 'good', 0.02, 0, 'snmp'),
(401, NOW() - INTERVAL 1 HOUR, -17.95, 2.33, -20.15, 2.50, 20.45, 3.30, 14.2, 40.8, 'good', -0.05, 0, 'snmp'),
(401, NOW(),                 -17.90, 2.35, -20.10, 2.50, 20.40, 3.31, 14.0, 40.3, 'good', 0.05, 0, 'snmp'),

(402, NOW() - INTERVAL 4 HOUR, -28.20, 2.05, -30.40, 2.50, 30.70, 3.22, 22.8, 53.0, 'critical', 0.00, 0, 'snmp'),
(402, NOW() - INTERVAL 3 HOUR, -28.35, 2.02, -30.55, 2.50, 30.85, 3.21, 23.1, 53.5, 'critical', -0.15, 0, 'snmp'),
(402, NOW() - INTERVAL 2 HOUR, -28.50, 2.00, -30.70, 2.50, 31.00, 3.20, 23.5, 54.0, 'critical', -0.15, 0, 'snmp'),
(402, NOW() - INTERVAL 1 HOUR, -28.62, 1.98, -30.82, 2.50, 31.12, 3.20, 23.8, 54.5, 'critical', -0.12, 0, 'snmp'),
(402, NOW(),                 -28.75, 1.95, -30.95, 2.50, 31.25, 3.19, 24.2, 55.0, 'critical', -0.13, 0, 'snmp');

-- Nokia
INSERT INTO onu_signal_history (onu_id, collected_at, rx_power_dbm, tx_power_dbm, olt_rx_power_dbm, olt_tx_power_dbm, attenuation_db, voltage_v, bias_current_ma, temperature_c, signal_quality, delta_prev_rx_db, is_degraded, collection_protocol)
VALUES
(501, NOW() - INTERVAL 4 HOUR, -16.10, 2.45, -18.40, 2.50, 18.60, 3.32, 13.5, 38.5, 'excellent', 0.00, 0, 'snmp'),
(501, NOW() - INTERVAL 3 HOUR, -16.12, 2.46, -18.42, 2.50, 18.62, 3.32, 13.6, 38.8, 'excellent', -0.02, 0, 'snmp'),
(501, NOW() - INTERVAL 2 HOUR, -16.10, 2.45, -18.40, 2.50, 18.60, 3.32, 13.5, 39.0, 'excellent', 0.02, 0, 'snmp'),
(501, NOW() - INTERVAL 1 HOUR, -16.15, 2.44, -18.45, 2.50, 18.65, 3.31, 13.7, 39.2, 'excellent', -0.05, 0, 'snmp'),
(501, NOW(),                 -16.10, 2.45, -18.40, 2.50, 18.60, 3.32, 13.5, 38.9, 'excellent', 0.05, 0, 'snmp'),

(502, NOW() - INTERVAL 4 HOUR, -19.80, 2.30, -22.00, 2.50, 22.30, 3.30, 15.5, 43.0, 'good', 0.00, 0, 'snmp'),
(502, NOW() - INTERVAL 3 HOUR, -19.85, 2.28, -22.05, 2.50, 22.35, 3.30, 15.6, 43.2, 'good', -0.05, 0, 'snmp'),
(502, NOW() - INTERVAL 2 HOUR, -19.82, 2.30, -22.02, 2.50, 22.32, 3.30, 15.5, 43.5, 'good', 0.03, 0, 'snmp'),
(502, NOW() - INTERVAL 1 HOUR, -19.90, 2.27, -22.10, 2.50, 22.40, 3.29, 15.8, 43.8, 'good', -0.08, 0, 'snmp'),
(502, NOW(),                 -19.85, 2.29, -22.05, 2.50, 22.35, 3.30, 15.6, 43.3, 'good', 0.05, 0, 'snmp');

-- Parks
INSERT INTO onu_signal_history (onu_id, collected_at, rx_power_dbm, tx_power_dbm, olt_rx_power_dbm, olt_tx_power_dbm, attenuation_db, voltage_v, bias_current_ma, temperature_c, signal_quality, delta_prev_rx_db, is_degraded, collection_protocol)
VALUES
(601, NOW() - INTERVAL 4 HOUR, -18.00, 2.30, -20.20, 2.50, 20.50, 3.30, 14.0, 41.0, 'good', 0.00, 0, 'snmp'),
(601, NOW() - INTERVAL 3 HOUR, -18.05, 2.28, -20.25, 2.50, 20.55, 3.30, 14.1, 41.2, 'good', -0.05, 0, 'snmp'),
(601, NOW() - INTERVAL 2 HOUR, -18.02, 2.30, -20.22, 2.50, 20.52, 3.30, 14.0, 41.5, 'good', 0.03, 0, 'snmp'),
(601, NOW() - INTERVAL 1 HOUR, -18.10, 2.27, -20.30, 2.50, 20.60, 3.29, 14.3, 41.8, 'good', -0.08, 0, 'snmp'),
(601, NOW(),                 -18.05, 2.29, -20.25, 2.50, 20.55, 3.30, 14.1, 41.3, 'good', 0.05, 0, 'snmp'),

(602, NOW() - INTERVAL 4 HOUR, -19.40, 2.40, -21.60, 2.50, 21.90, 3.30, 15.2, 43.5, 'good', 0.00, 0, 'snmp'),
(602, NOW() - INTERVAL 3 HOUR, -19.48, 2.38, -21.68, 2.50, 21.98, 3.29, 15.4, 43.8, 'good', -0.08, 0, 'snmp'),
(602, NOW() - INTERVAL 2 HOUR, -22.10, 2.35, -24.30, 2.50, 24.60, 3.28, 17.1, 45.5, 'warning', -2.62, 1, 'snmp'),
(602, NOW() - INTERVAL 1 HOUR, -23.50, 2.30, -25.70, 2.50, 26.00, 3.27, 18.3, 46.8, 'warning', -1.40, 1, 'snmp'),
(602, NOW(),                 -23.90, 2.28, -26.10, 2.50, 26.40, 3.26, 18.9, 47.0, 'warning', -0.40, 1, 'snmp');

-- TP-Link
INSERT INTO onu_signal_history (onu_id, collected_at, rx_power_dbm, tx_power_dbm, olt_rx_power_dbm, olt_tx_power_dbm, attenuation_db, voltage_v, bias_current_ma, temperature_c, signal_quality, delta_prev_rx_db, is_degraded, collection_protocol)
VALUES
(1001, NOW() - INTERVAL 4 HOUR, -16.45, 2.30, -19.20, 2.50, 18.95, 3.31, 14.50, 42.1, 'excellent', 0.00, 0, 'snmp'),
(1001, NOW() - INTERVAL 3 HOUR, -16.48, 2.32, -19.25, 2.50, 18.98, 3.30, 14.60, 42.5, 'excellent', -0.03, 0, 'snmp'),
(1001, NOW() - INTERVAL 2 HOUR, -16.50, 2.30, -19.22, 2.50, 19.00, 3.31, 14.50, 43.0, 'excellent', -0.02, 0, 'snmp'),
(1001, NOW() - INTERVAL 1 HOUR, -16.52, 2.31, -19.30, 2.50, 19.02, 3.30, 14.70, 43.2, 'excellent', -0.02, 0, 'snmp'),
(1001, NOW(),                 -16.50, 2.30, -19.25, 2.50, 19.00, 3.31, 14.50, 42.8, 'excellent',  0.02, 0, 'snmp'),

(1002, NOW() - INTERVAL 4 HOUR, -19.90, 2.45, -22.10, 2.50, 22.40, 3.29, 16.20, 45.0, 'good', 0.00, 0, 'snmp'),
(1002, NOW() - INTERVAL 3 HOUR, -19.95, 2.40, -22.15, 2.50, 22.45, 3.30, 16.10, 45.2, 'good', -0.05, 0, 'snmp'),
(1002, NOW() - INTERVAL 2 HOUR, -20.02, 2.42, -22.20, 2.50, 22.52, 3.28, 16.30, 46.1, 'good', -0.07, 0, 'snmp'),
(1002, NOW() - INTERVAL 1 HOUR, -20.08, 2.40, -22.25, 2.50, 22.58, 3.29, 16.20, 45.8, 'good', -0.06, 0, 'snmp'),
(1002, NOW(),                 -20.12, 2.41, -22.30, 2.50, 22.62, 3.29, 16.40, 45.5, 'good', -0.04, 0, 'snmp'),

(1003, NOW() - INTERVAL 4 HOUR, -27.80, 2.10, -29.90, 2.50, 30.30, 3.25, 22.50, 51.2, 'critical', 0.00, 0, 'snmp'),
(1003, NOW() - INTERVAL 3 HOUR, -28.00, 2.12, -30.10, 2.50, 30.50, 3.24, 22.80, 52.0, 'critical', -0.20, 0, 'snmp'),
(1003, NOW() - INTERVAL 2 HOUR, -28.15, 2.10, -30.25, 2.50, 30.65, 3.25, 23.10, 52.5, 'critical', -0.15, 0, 'snmp'),
(1003, NOW() - INTERVAL 1 HOUR, -28.30, 2.08, -30.40, 2.50, 30.80, 3.23, 23.40, 53.1, 'critical', -0.15, 0, 'snmp'),
(1003, NOW(),                 -28.45, 2.05, -30.60, 2.50, 30.95, 3.22, 23.90, 53.8, 'critical', -0.15, 1, 'snmp'),

(1004, NOW() - INTERVAL 4 HOUR, -19.20, 2.50, -21.40, 2.50, 21.70, 3.30, 15.00, 41.0, 'good',     0.00, 0, 'snmp'),
(1004, NOW() - INTERVAL 3 HOUR, -19.25, 2.48, -21.45, 2.50, 21.75, 3.31, 15.10, 41.2, 'good',    -0.05, 0, 'snmp'),
(1004, NOW() - INTERVAL 2 HOUR, -21.50, 2.45, -23.80, 2.50, 24.00, 3.29, 16.50, 43.0, 'good',    -2.25, 0, 'snmp'),
(1004, NOW() - INTERVAL 1 HOUR, -23.10, 2.40, -25.50, 2.50, 25.60, 3.28, 17.80, 44.5, 'warning', -1.60, 1, 'snmp'),
(1004, NOW(),                 -23.50, 2.38, -26.00, 2.50, 26.00, 3.27, 18.20, 45.0, 'warning', -0.40, 1, 'snmp'),

(1005, NOW() - INTERVAL 4 HOUR, -18.10, 2.35, -20.30, 2.50, 20.60, 3.32, 14.80, 40.5, 'good', 0.00, 0, 'snmp'),
(1005, NOW() - INTERVAL 3 HOUR, -18.15, 2.32, -20.35, 2.50, 20.65, 3.31, 14.90, 40.8, 'good', -0.05, 0, 'snmp'),
(1005, NOW() - INTERVAL 2 HOUR, -18.18, 2.35, -20.40, 2.50, 20.68, 3.30, 14.80, 41.2, 'good', -0.03, 0, 'snmp'),
(1005, NOW() - INTERVAL 1 HOUR, -18.22, 2.30, -20.45, 2.50, 20.72, 3.32, 15.00, 41.5, 'good', -0.04, 0, 'snmp'),
(1005, NOW(),                 -18.20, 2.34, -20.40, 2.50, 20.70, 3.31, 14.90, 41.0, 'good',  0.02, 0, 'snmp'),

(1006, NOW() - INTERVAL 4 HOUR, -25.50, 2.20, -27.80, 2.50, 28.00, 3.26, 19.50, 48.0, 'warning', 0.00, 0, 'snmp'),
(1006, NOW() - INTERVAL 3 HOUR, -25.60, 2.18, -27.90, 2.50, 28.10, 3.25, 19.70, 48.2, 'warning', -0.10, 0, 'snmp'),
(1006, NOW() - INTERVAL 2 HOUR, -25.72, 2.20, -28.05, 2.50, 28.22, 3.26, 19.90, 48.9, 'warning', -0.12, 0, 'snmp'),
(1006, NOW() - INTERVAL 1 HOUR, -25.80, 2.15, -28.15, 2.50, 28.30, 3.24, 20.10, 49.3, 'warning', -0.08, 0, 'snmp'),
(1006, NOW(),                 -25.85, 2.16, -28.20, 2.50, 28.35, 3.25, 20.30, 49.0, 'warning', -0.05, 0, 'snmp');

-- -----------------------------------------------------------------------------
-- 4. REGISTROS DE AUDITORIA
-- -----------------------------------------------------------------------------
INSERT INTO audit_logs (user_id, action, resource_type, resource_id, details, ip_address, created_at)
VALUES
(1, 'COLLECT_SNMP', 'OLT', '1', 'Varredura 100% SNMPv2c concluída na OLT-ZTE-TITAN-01 (4 ONUs)', '127.0.0.1', NOW() - INTERVAL 30 MINUTE),
(1, 'COLLECT_SNMP', 'OLT', '2', 'Varredura 100% SNMPv2c concluída na OLT-HUAWEI-MA5800-01 (3 ONUs)', '127.0.0.1', NOW() - INTERVAL 25 MINUTE),
(1, 'COLLECT_SNMP', 'OLT', '3', 'Varredura 100% SNMPv2c concluída na OLT-DATACOM-DM4610-01 (2 ONUs)', '127.0.0.1', NOW() - INTERVAL 20 MINUTE),
(1, 'COLLECT_SNMP', 'OLT', '4', 'Varredura 100% SNMPv2c concluída na OLT-FIBERHOME-5516-01 (2 ONUs)', '127.0.0.1', NOW() - INTERVAL 15 MINUTE),
(1, 'COLLECT_SNMP', 'OLT', '5', 'Varredura 100% SNMPv2c concluída na OLT-NOKIA-ISAM-01 (2 ONUs)', '127.0.0.1', NOW() - INTERVAL 10 MINUTE),
(1, 'COLLECT_SNMP', 'OLT', '6', 'Varredura 100% SNMPv2c concluída na OLT-PARKS-FIBERLINK-01 (2 ONUs)', '127.0.0.1', NOW() - INTERVAL 5 MINUTE),
(1, 'COLLECT_SNMP', 'OLT', '10', 'Varredura 100% SNMPv2c concluída na OLT-TPLINK-CENTRAL-01 (8 ONUs)', '127.0.0.1', NOW() - INTERVAL 2 MINUTE);

SET FOREIGN_KEY_CHECKS = 1;
