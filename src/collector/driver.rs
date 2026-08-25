use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// Modelo padronizado de leitura óptica de uma ONU / ONT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnuOpticalData {
    pub slot: i32,
    pub pon_port: i32,
    pub onu_id: i32,
    pub serial_number: String,
    pub customer_identifier: Option<String>,

    // Potências Ópticas (dBm)
    pub rx_power_dbm: Option<f64>,     // Sinal recebido pela ONU da OLT
    pub tx_power_dbm: Option<f64>,     // Sinal emitido pelo laser da ONU
    pub olt_rx_power_dbm: Option<f64>, // Sinal da ONU recebido na porta PON da OLT
    pub olt_tx_power_dbm: Option<f64>, // Potência de saída do GBIC PON da OLT

    // Diagnósticos DDM
    pub attenuation_db: Option<f64>, // Perda óptica total (OLT Tx - ONU Rx)
    pub temperature_c: Option<f64>,  // Temperatura da ONU em °C
    pub voltage_v: Option<f64>,      // Tensão de alimentação interna
    pub bias_current_ma: Option<f64>, // Corrente de polarização do laser (mA)

    // Distância física em metros (se fornecida pela OLT)
    pub distance_meters: Option<i32>,
    pub is_online: bool,
    pub offline_reason: Option<String>, // "dying_gasp", "los", "manual_deactivate", etc.
}

/// Parâmetros de conexão e limites de segurança de uma OLT
#[derive(Debug, Clone)]
pub struct OltTarget {
    pub id: u64,
    pub name: String,
    pub ip_address: String,
    pub vendor: String,
    pub model: Option<String>,

    // Parâmetros SNMP
    pub snmp_version: String,
    pub snmp_port: u16,
    pub snmp_community: Option<String>,

    // Proteção de CPU da OLT
    pub max_concurrent_requests: usize,
    pub pon_delay: Duration,
    pub timeout: Duration,
}

/// Trait unificada que todo driver de fabricante deve implementar
#[async_trait::async_trait]
pub trait OltDriver: Send + Sync {
    /// Nome do fabricante atendido pelo driver
    fn vendor_name(&self) -> &'static str;

    /// Coleta todos os sinais ópticos das ONUs respeitando semáforo e rate limiting
    async fn collect_optical_signals(
        &self,
        target: &OltTarget,
        semaphore: Arc<Semaphore>,
    ) -> Result<Vec<OnuOpticalData>, Box<dyn std::error::Error + Send + Sync>>;

    /// Teste rápido de conectividade e leitura de versão de firmware
    async fn test_connectivity(
        &self,
        target: &OltTarget,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;
}
