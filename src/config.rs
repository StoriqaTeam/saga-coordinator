use std::env;
use std::net::SocketAddr;

use config_crate::{Config as RawConfig, ConfigError, Environment, File};

use stq_logging::GrayLogConfig;
use stq_routes::service::Service as StqService;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub listen: SocketAddr,
    pub users_addr: String,
    pub stores_addr: String,
    pub warehouses_addr: String,
    pub orders_addr: String,
    pub billing_addr: String,
    pub notifications_addr: String,
    pub graylog: Option<GrayLogConfig>,
}

impl Config {
    /// Creates config from base.toml, which are overwritten by <env>.toml, where
    /// env is one of development, test, production. After that it could be overwritten
    /// by environment variables like STQ_SAGA_LISTEN (this will override `listen` field in config)
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = RawConfig::new();

        s.merge(File::with_name("config/base"))?;

        // Optional file specific for environment
        let env = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        s.merge(File::with_name(&format!("config/{}", env.to_string())).required(false))?;

        // Add in settings from the environment (with a prefix of STQ_SAGA)
        s.merge(Environment::with_prefix("STQ_SAGA"))?;

        s.try_into()
    }

    pub fn service_url(&self, service: StqService) -> String {
        match service {
            StqService::Users => self.users_addr.clone(),
            StqService::Stores => self.stores_addr.clone(),
            StqService::Warehouses => self.warehouses_addr.clone(),
            StqService::Orders => self.orders_addr.clone(),
            StqService::Billing => self.billing_addr.clone(),
            StqService::Notifications => self.notifications_addr.clone(),
        }
    }
}
