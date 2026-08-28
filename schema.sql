-- ==============================================================================
-- SignalHunter: Esquema de Banco de Dados Relacional (MySQL / MariaDB)
-- Otimizado para Séries Temporais de Sinais Ópticos e Auditoria de FTTH
-- ==============================================================================

SET FOREIGN_KEY_CHECKS = 0;

-- 1. Tabela de Usuários e Controle de Acesso (RBAC)
CREATE TABLE IF NOT EXISTS users (
    id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    username VARCHAR(64) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    full_name VARCHAR(128) NOT NULL,
    email VARCHAR(128) NULL,
    role ENUM('admin', 'operator', 'viewer') NOT NULL DEFAULT 'operator',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 2. Tabela de OLTs (Equipamentos)
CREATE TABLE IF NOT EXISTS olts (
    id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(128) NOT NULL,
    ip_address VARCHAR(45) NOT NULL,
    vendor ENUM('huawei', 'zte', 'datacom', 'fiberhome', 'nokia', 'parks', 'tplink', 'generic') NOT NULL,
    model VARCHAR(64) NULL,
    firmware_version VARCHAR(64) NULL,
    
    -- Parâmetros SNMPv2c (Credenciais cifradas com AES-256-GCM)
    snmp_port INT UNSIGNED NOT NULL DEFAULT 161,
    snmp_community_encrypted TEXT NULL,
    
    -- Parâmetros de Proteção e Agendamento
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    collection_interval_mins INT UNSIGNED NOT NULL DEFAULT 60,
    max_concurrent_requests TINYINT UNSIGNED NOT NULL DEFAULT 2, -- Proteção CPU por chassi
    pon_delay_ms INT UNSIGNED NOT NULL DEFAULT 50,              -- Pacing entre PONs
    
    -- Status da Última Coleta
    last_collected_at DATETIME NULL,
    last_collection_status ENUM('success', 'partial_error', 'failed', 'in_progress', 'never') NOT NULL DEFAULT 'never',
    last_error_message TEXT NULL,
    
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_olt_active (is_active),
    INDEX idx_olt_scheduler (is_active, last_collected_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 3. Tabela Unificada de ONUs / ONTs
CREATE TABLE IF NOT EXISTS onus (
    id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    olt_id BIGINT UNSIGNED NOT NULL,
    slot INT NOT NULL,
    pon_port INT NOT NULL,
    onu_id INT NOT NULL,
    serial_number VARCHAR(64) NOT NULL, -- Ex: HWTC12345678, ZTEGC1234567
    mac_address VARCHAR(17) NULL,
    model VARCHAR(64) NULL,
    custom_name VARCHAR(128) NULL,
    customer_identifier VARCHAR(128) NULL, -- ID do cliente / Login PPPoE / Contrato
    distance_meters INT NULL,
    status ENUM('online', 'los', 'dying_gasp', 'offline', 'unknown') NOT NULL DEFAULT 'unknown',
    
    first_seen_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (olt_id) REFERENCES olts(id) ON DELETE CASCADE,
    UNIQUE KEY uk_olt_serial (olt_id, serial_number),
    INDEX idx_olt_slot_port (olt_id, slot, pon_port, onu_id),
    INDEX idx_onu_customer (customer_identifier),
    INDEX idx_onu_status (status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 4. Histórico de Leituras de Sinais (Série Temporal)
CREATE TABLE IF NOT EXISTS onu_signal_history (
    id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    onu_id BIGINT UNSIGNED NOT NULL,
    collected_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    -- Potências Ópticas (dBm com 2 casas decimais)
    rx_power_dbm DECIMAL(5,2) NULL, -- Sinal recebido pela ONU da OLT
    tx_power_dbm DECIMAL(5,2) NULL, -- Sinal emitido pela ONU
    olt_rx_power_dbm DECIMAL(5,2) NULL, -- Sinal da ONU recebido pela OLT
    olt_tx_power_dbm DECIMAL(5,2) NULL, -- Sinal emitido pelo GBIC da OLT
    
    -- Diagnóstico Óptico e DDM
    attenuation_db DECIMAL(5,2) NULL, -- Perda óptica calculada (OLT Tx - ONU Rx)
    voltage_v DECIMAL(4,2) NULL,
    bias_current_ma DECIMAL(5,2) NULL,
    temperature_c DECIMAL(4,1) NULL,
    
    -- Classificação e Variação Histórica
    signal_quality ENUM('excellent', 'good', 'warning', 'critical', 'offline') NOT NULL DEFAULT 'good',
    delta_prev_rx_db DECIMAL(4,2) NULL, -- Diferença em dB em relação à última leitura
    is_degraded BOOLEAN NOT NULL DEFAULT FALSE,
    
    -- Metadados da Coleta (100% SNMPv2c)
    collection_protocol ENUM('snmp') NOT NULL DEFAULT 'snmp',
    response_time_ms INT UNSIGNED NULL,
    
    FOREIGN KEY (onu_id) REFERENCES onus(id) ON DELETE CASCADE,
    INDEX idx_history_onu_time (onu_id, collected_at),
    INDEX idx_history_quality_time (signal_quality, collected_at),
    INDEX idx_history_degraded (is_degraded, collected_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 5. Limiares Configuráveis de Qualidade Óptica
CREATE TABLE IF NOT EXISTS signal_thresholds (
    id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(64) NOT NULL DEFAULT 'Padrão GPON / EPON',
    rx_excellent_min DECIMAL(5,2) NOT NULL DEFAULT -18.00,
    rx_excellent_max DECIMAL(5,2) NOT NULL DEFAULT -14.00,
    rx_good_min DECIMAL(5,2) NOT NULL DEFAULT -23.00,
    rx_good_max DECIMAL(5,2) NOT NULL DEFAULT -8.00,
    rx_warning_min DECIMAL(5,2) NOT NULL DEFAULT -26.90,
    rx_critical_min DECIMAL(5,2) NOT NULL DEFAULT -27.00,
    degradation_alert_delta_db DECIMAL(4,2) NOT NULL DEFAULT 3.00,
    is_default BOOLEAN NOT NULL DEFAULT TRUE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 6. Histórico de Relatórios Gerados em PDF
CREATE TABLE IF NOT EXISTS generated_reports (
    id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    report_type ENUM('olt_summary', 'critical_onus', 'degradation_trend', 'full_inventory') NOT NULL,
    olt_id BIGINT UNSIGNED NULL,
    generated_by BIGINT UNSIGNED NULL,
    file_path VARCHAR(512) NOT NULL,
    total_onus_analyzed INT UNSIGNED NOT NULL DEFAULT 0,
    critical_count INT UNSIGNED NOT NULL DEFAULT 0,
    warning_count INT UNSIGNED NOT NULL DEFAULT 0,
    degraded_count INT UNSIGNED NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (olt_id) REFERENCES olts(id) ON DELETE SET NULL,
    FOREIGN KEY (generated_by) REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 7. Logs de Auditoria do Sistema
CREATE TABLE IF NOT EXISTS audit_logs (
    id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    user_id BIGINT UNSIGNED NULL,
    action VARCHAR(64) NOT NULL,
    resource_type VARCHAR(64) NOT NULL,
    resource_id VARCHAR(64) NULL,
    details TEXT NULL,
    ip_address VARCHAR(45) NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL,
    INDEX idx_audit_time (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

SET FOREIGN_KEY_CHECKS = 1;

-- Inserção de dados padrão iniciais
INSERT IGNORE INTO signal_thresholds (id, name, rx_excellent_min, rx_excellent_max, rx_good_min, rx_good_max, rx_warning_min, rx_critical_min, degradation_alert_delta_db, is_default)
VALUES (1, 'Padrão GPON / EPON ISP', -18.00, -14.00, -23.00, -8.00, -26.90, -27.00, 3.00, TRUE);
