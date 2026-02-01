use titan_rust_client::types::SwapRoute;

pub fn select_best_route(routes: impl IntoIterator<Item = SwapRoute>) -> Option<SwapRoute> {
    routes.into_iter().max_by_key(|r| r.out_amount)
}
