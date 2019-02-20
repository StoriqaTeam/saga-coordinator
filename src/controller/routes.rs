use stq_router::RouteParser;
use stq_types::{BaseProductId, OrderId, OrderSlug, ProductId, StoreId};

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
    StoreModerate,
    StoreModeration(StoreId),
    StoreDeactivate(StoreId),
    BaseProductUpdate(BaseProductId),
    BaseProductCreateWithVariants,
    BaseProductModerate,
    BaseProductDeactivate(BaseProductId),
    BaseProductUpsertShipping(BaseProductId),
    BaseProductModeration(BaseProductId),
    ProductDeactivate(ProductId),
    OrdersSetPaymentState { order_id: OrderId },
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

    router.add_route(r"^/stores/moderate$", || Route::StoreModerate);

    router.add_route_with_params(r"^/stores/(\d+)/moderation$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse::<StoreId>().ok())
            .map(Route::StoreModeration)
    });

    router.add_route_with_params(r"^/stores/(\d+)/deactivate$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse::<StoreId>().ok())
            .map(Route::StoreDeactivate)
    });

    router.add_route(r"^/base_products/moderate$", || Route::BaseProductModerate);

    router.add_route_with_params(r"^/base_products/(\d+)/moderation$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse::<BaseProductId>().ok())
            .map(Route::BaseProductModeration)
    });

    router.add_route_with_params(r"^/base_products/(\d+)/deactivate$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse::<BaseProductId>().ok())
            .map(Route::BaseProductDeactivate)
    });

    router.add_route_with_params(r"^/base_products/(\d+)/update$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse::<BaseProductId>().ok())
            .map(Route::BaseProductUpdate)
    });

    router.add_route(r"^/base_products/create_with_variants$", || Route::BaseProductCreateWithVariants);

    router.add_route_with_params(r"^/base_products/(\d+)/upsert-shipping$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse::<BaseProductId>().ok())
            .map(Route::BaseProductUpsertShipping)
    });

    router.add_route_with_params(r"^/products/(\d+)/deactivate$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse::<ProductId>().ok())
            .map(Route::ProductDeactivate)
    });

    router.add_route(r"^/orders/update_state$", || Route::OrdersUpdateStateByBilling);

    router.add_route_with_params(r"^/orders/(\d+)/set_state$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|order_slug| Route::OrdersManualSetState { order_slug })
    });

    router.add_route_with_params(r"^/orders/([a-zA-Z0-9-]+)/set_payment_state$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|order_id| Route::OrdersSetPaymentState { order_id })
    });

    router
}
