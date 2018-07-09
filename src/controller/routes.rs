use stq_router::RouteParser;

#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    Healthcheck,
    CreateAccount,
    CreateStore,
    CreateOrder,
}

pub fn create_route_parser() -> RouteParser<Route> {
    let mut router = RouteParser::default();

    // Healthcheck
    router.add_route(r"^/healthcheck$", || Route::Healthcheck);

    router.add_route(r"^/create_account", || Route::CreateAccount);

    router.add_route(r"^/create_store", || Route::CreateStore);

    router.add_route(r"^/create_order", || Route::CreateOrder);

    router
}
