use crate::common::{
    build_quote_request, build_swap_config, finalize_swap, load_test_env, print_quote,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signer::Signer;
use solana_swap::{Provider, SwapAggregator};

#[tokio::test]
#[ignore = "requires env vars and real API access"]
async fn test_dflow_max_route_length() {
    let env = load_test_env();
    let aggregator = SwapAggregator::new(build_swap_config(&env, Some(1)));
    let request = build_quote_request(&env, None);
    let rpc_client = RpcClient::new(env.rpc_url.clone());
    let pubkey = env.keypair.pubkey();

    let quote = aggregator
        .quote(Provider::Dflow, &request)
        .await
        .expect("dflow max_route_length=1 quote should succeed");

    assert_eq!(quote.provider, Provider::Dflow);
    assert!(quote.output_amount > 0, "output_amount must be > 0");
    assert_eq!(quote.input_mint, env.input_mint);
    assert_eq!(quote.output_mint, env.output_mint);

    print_quote("dflow::max_route_length", &quote);

    let swap_result = aggregator
        .swap(&quote, &pubkey, &rpc_client)
        .await
        .expect("dflow max_route_length=1 swap should succeed");

    finalize_swap(
        "dflow::max_route_length",
        swap_result,
        &env.keypair,
        &rpc_client,
        env.send_tx,
    )
    .await;
}
