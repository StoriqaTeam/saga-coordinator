use stq_http;

use stq_router::service::Service as StqService;

#[derive(Clone, Debug)]
pub struct Config {
    pub users_addr: String,
    pub stores_addr: String,
}

impl Config {
    pub fn service_url(&self, service: StqService) -> String {
        match service {
            StqService::Users => self.users_addr.clone(),
            StqService::Stores => self.stores_addr.clone(),
        }
    }
}
