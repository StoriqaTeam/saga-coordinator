mod routes;

use config::Config;
use self::routes::Route;

use std::sync::Arc;
use stq_http::{ClientHandle, router::RouteParser};

pub struct Controller {
    pub route_parser: Arc<RouteParser<Route>>,
    pub config: Config,
    pub client_handle: ClientHandle,
}
