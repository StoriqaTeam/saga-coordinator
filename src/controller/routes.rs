use stq_router::RouteParser;

#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    CreateAccount,
}

pub fn create_router() -> RouteParser<Route> {
    let mut router = RouteParser::default();

    router.add_route(r"^/create_account", || Route::CreateAccount);

    router
}
