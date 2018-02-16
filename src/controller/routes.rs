use stq_http::router::RouteParser;

#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    CreateAccount,
}

pub fn create_router() -> RouteParser<Route> {
    let mut router = RouteParser::new();

    router.add_route(r"^/create_account", || Route::CreateAccount);

    router
}
