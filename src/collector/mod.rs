pub mod driver;
pub mod snmp;
pub mod vendors;

use crate::collector::driver::{OltDriver, OltTarget, OnuOpticalData};
use log::{info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct CollectorRegistry {
    drivers: HashMap<String, Box<dyn OltDriver>>,
}

impl CollectorRegistry {
    pub fn new() -> Self {
        Self {
            drivers: HashMap::new(),
        }
    }

    pub fn register<D: OltDriver + 'static>(&mut self, driver: D) {
        let vendor = driver.vendor_name().to_lowercase();
        self.drivers.insert(vendor, Box::new(driver));
    }

    pub async fn execute_scan(
        &self,
        target: &OltTarget,
    ) -> Result<Vec<OnuOpticalData>, Box<dyn std::error::Error + Send + Sync>> {
        let vendor_key = target.vendor.to_lowercase();
        let driver = self.drivers.get(&vendor_key).ok_or_else(|| {
            format!(
                "Nenhum driver de coleta registrado para a fabricante: '{}'",
                target.vendor
            )
        })?;

        // Semáforo de contenção de CPU por chassi de OLT
        let max_concurrency = target.max_concurrent_requests.clamp(1, 4);
        let semaphore = Arc::new(Semaphore::new(max_concurrency));

        info!(
            "Iniciando coleta controlada para OLT '{}' ({}) via protocolo primário '{}' (Concorrência máxima: {})",
            target.name, target.ip_address, target.primary_protocol, max_concurrency
        );

        match driver
            .collect_optical_signals(target, semaphore.clone())
            .await
        {
            Ok(data) => {
                info!(
                    "Coleta finalizada com sucesso para OLT '{}'. Total ONUs: {}",
                    target.name,
                    data.len()
                );
                Ok(data)
            }
            Err(e) => {
                warn!("Falha na coleta da OLT '{}': {:?}", target.name, e);
                Err(e)
            }
        }
    }
}
