use stq_router::RouteParser;
use stq_types::OrderSlug;

#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    CreateAccount,
    VerifyEmail,
    VerifyEmailApply,
    ResetPassword,
    ResetPasswordApply,
    CreateStore,
    CreateOrder,
    BuyNow,
    OrdersUpdateStateByBilling,
    OrdersManualSetState { order_slug: OrderSlug },
}

pub fn create_route_parser() -> RouteParser<Route> {
    let mut router = RouteParser::default();

    router.add_route(r"^/create_account$", || Route::CreateAccount);

    router.add_route(r"^/email_verify$", || Route::VerifyEmail);

    router.add_route(r"^/email_verify_apply$", || Route::VerifyEmailApply);

    router.add_route(r"^/reset_password$", || Route::ResetPassword);

    router.add_route(r"^/reset_password_apply$", || Route::ResetPasswordApply);

    router.add_route(r"^/create_store$", || Route::CreateStore);

    router.add_route(r"^/create_order$", || Route::CreateOrder);

    router.add_route(r"^/buy_now$", || Route::BuyNow);

    router.add_route(r"^/orders/update_state$", || Route::OrdersUpdateStateByBilling);

    router.add_route_with_params(r"^/orders/(\d+)/set_state$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|order_slug| Route::OrdersManualSetState { order_slug })
    });

    router
}
