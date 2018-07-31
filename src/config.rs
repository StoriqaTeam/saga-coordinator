use std::env;

use config_crate::{Config as RawConfig, ConfigError, Environment, File};

use stq_logging::GrayLogConfig;
use stq_routes::service::Service as StqService;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub server: Server,
    pub users_microservice: Microservice,
    pub stores_microservice: Microservice,
    pub orders_microservice: Microservice,
    pub billing_microservice: Microservice,
    pub warehouses_microservice: Microservice,
    pub notifications_microservice: Microservice,
    pub graylog: Option<GrayLogConfig>,
    pub cluster: Cluster,
    pub notification_urls: NotificationUrls,
}

/// Common server settings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Server {
    pub host: String,
    pub port: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Microservice {
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cluster {
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotificationUrls {
    pub verify_email_path: String,
    pub reset_password_path: String,
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
            StqService::Users => self.users_microservice.url.clone(),
            StqService::Stores => self.stores_microservice.url.clone(),
            StqService::Warehouses => self.warehouses_microservice.url.clone(),
            StqService::Orders => self.orders_microservice.url.clone(),
            StqService::Billing => self.billing_microservice.url.clone(),
            StqService::Notifications => self.notifications_microservice.url.clone(),
        }
    }
}
