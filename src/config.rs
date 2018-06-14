use std::env;
use std::net::SocketAddr;

use stq_routes::service::Service as StqService;

use config_crate::{Config as RawConfig, ConfigError, Environment, File};

enum Env {
    Development,
    Test,
    Production,
}

impl Env {
    fn new() -> Self {
        match env::var("RUN_MODE") {
            Ok(ref s) if s == "test" => Env::Test,
            Ok(ref s) if s == "production" => Env::Production,
            _ => Env::Development,
        }
    }

    fn to_string(&self) -> &'static str {
        match self {
            &Env::Development => "development",
            &Env::Production => "production",
            &Env::Test => "test",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub listen: SocketAddr,
    pub users_addr: String,
    pub stores_addr: String,
    pub warehouses_addr: String,
}

impl Config {
    /// Creates config from base.toml, which are overwritten by <env>.toml, where
    /// env is one of development, test, production. After that it could be overwritten
    /// by environment variables like STQ_SEC_LISTEN (this will override `listen` field in config)
    pub fn new() -> Result<Self, ConfigError> {
        let env = Env::new();
        let mut s = RawConfig::new();

        s.merge(File::with_name("config/base"))?;
        // Optional file specific for environment
        s.merge(File::with_name(&format!("config/{}", env.to_string())).required(false))?;

        // Add in settings from the environment (with a prefix of STQ_SEC)
        s.merge(Environment::with_prefix("STQ_SEC"))?;

        s.try_into()
    }

    pub fn service_url(&self, service: StqService) -> String {
        match service {
            StqService::Users => self.users_addr.clone(),
            StqService::Stores => self.stores_addr.clone(),
            StqService::Warehouses => self.warehouses_addr.clone(),
            _ => "".to_string(), // other services are not required
        }
    }
}
