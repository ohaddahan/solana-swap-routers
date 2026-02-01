#![allow(
    clippy::unwrap_used,
    reason = "test code — panicking on failure is expected"
)]
#![allow(
    clippy::expect_used,
    reason = "test code — panicking on failure is expected"
)]
#![allow(clippy::panic, reason = "test code — panicking on failure is expected")]
#![allow(clippy::map_unwrap_or, reason = "readability in test setup helpers")]

use std::str::FromStr;

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{hash::Hash, pubkey::Pubkey, signature::Keypair, signer::Signer};

use solana_swap::{Provider, QuoteRequest, SwapAggregator, SwapConfig, SwapResult};

struct TestEnv {
    input_mint: Pubkey,
    output_mint: Pubkey,
    amount: u64,
    slippage_bps: u16,
    keypair: Keypair,
    rpc_url: String,
    jupiter_api_key: Option<String>,
    titan_ws_url: Option<String>,
    titan_token: Option<String>,
    dflow_api_key: Option<String>,
}

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("env var {name} is required but not set"))
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn load_test_env() -> TestEnv {
    dotenvy::dotenv().ok();

    let input_mint = Pubkey::from_str(&required_env("TEST_INPUT_MINT"))
        .expect("TEST_INPUT_MINT must be a valid pubkey");
    let output_mint = Pubkey::from_str(&required_env("TEST_OUTPUT_MINT"))
        .expect("TEST_OUTPUT_MINT must be a valid pubkey");
    let keypair_path = required_env("TEST_KEYPAIR_PATH");
    let rpc_url = required_env("TEST_RPC_URL");

    let amount = optional_env("TEST_AMOUNT")
        .map(|v| v.parse::<u64>().expect("TEST_AMOUNT must be a valid u64"))
        .unwrap_or(1000);
    let slippage_bps = optional_env("TEST_SLIPPAGE_BPS")
        .map(|v| {
            v.parse::<u16>()
                .expect("TEST_SLIPPAGE_BPS must be a valid u16")
        })
        .unwrap_or(300);

    let keypair_bytes = std::fs::read_to_string(&keypair_path)
        .unwrap_or_else(|e| panic!("failed to read keypair at {keypair_path}: {e}"));
    let keypair_vec: Vec<u8> = serde_json::from_str(&keypair_bytes)
        .unwrap_or_else(|e| panic!("failed to parse keypair JSON at {keypair_path}: {e}"));
    let keypair = Keypair::try_from(keypair_vec.as_slice())
        .unwrap_or_else(|e| panic!("invalid keypair bytes at {keypair_path}: {e}"));

    TestEnv {
        input_mint,
        output_mint,
        amount,
        slippage_bps,
        keypair,
        rpc_url,
        jupiter_api_key: optional_env("TEST_JUPITER_API_KEY"),
        titan_ws_url: optional_env("TEST_TITAN_WS_URL"),
        titan_token: optional_env("TEST_TITAN_TOKEN"),
        dflow_api_key: optional_env("TEST_DFLOW_API_KEY"),
    }
}

fn build_swap_config(env: &TestEnv) -> SwapConfig {
    SwapConfig {
        default_slippage_bps: env.slippage_bps,
        jupiter_api_url: None,
        jupiter_api_key: env.jupiter_api_key.clone(),
        titan_ws_url: env.titan_ws_url.clone(),
        titan_token: env.titan_token.clone(),
        dflow_api_url: None,
        dflow_api_key: env.dflow_api_key.clone(),
        dflow_max_route_length: None,
    }
}

fn build_quote_request(env: &TestEnv) -> QuoteRequest {
    QuoteRequest {
        input_mint: env.input_mint,
        output_mint: env.output_mint,
        amount: env.amount,
        slippage_bps: Some(env.slippage_bps),
    }
}

fn assert_swap_result_valid(result: SwapResult, payer: &Pubkey) {
    match &result {
        SwapResult::Instructions { instructions, .. } => {
            assert!(
                !instructions.is_empty(),
                "expected at least one instruction"
            );
        }
        SwapResult::Transaction { transaction, .. } => {
            assert!(
                !transaction.message.instructions().is_empty(),
                "expected at least one instruction in transaction"
            );
        }
    }

    let tx = result
        .into_unsigned_transaction(payer, Hash::default())
        .expect("into_unsigned_transaction should succeed");

    assert!(
        !tx.message.instructions().is_empty(),
        "unsigned transaction should have instructions"
    );
}

#[tokio::test]
#[ignore = "requires env vars and real API access"]
async fn test_jupiter_quote_and_swap() {
    let env = load_test_env();
    let aggregator = SwapAggregator::new(build_swap_config(&env));
    let request = build_quote_request(&env);
    let rpc_client = RpcClient::new(env.rpc_url.clone());
    let pubkey = env.keypair.pubkey();

    let quote = aggregator
        .quote(Provider::Jupiter, &request)
        .await
        .expect("jupiter quote should succeed");

    assert_eq!(quote.provider, Provider::Jupiter);
    assert!(quote.output_amount > 0, "output_amount must be > 0");
    assert_eq!(quote.input_mint, env.input_mint);
    assert_eq!(quote.output_mint, env.output_mint);

    let swap_result = aggregator
        .swap(&quote, &pubkey, &rpc_client)
        .await
        .expect("jupiter swap should succeed");

    assert_swap_result_valid(swap_result, &pubkey);
}

#[tokio::test]
#[ignore = "requires env vars and real API access"]
async fn test_titan_quote_and_swap() {
    let env = load_test_env();
    let aggregator = SwapAggregator::new(build_swap_config(&env));
    let request = build_quote_request(&env);
    let rpc_client = RpcClient::new(env.rpc_url.clone());
    let pubkey = env.keypair.pubkey();

    let quote = aggregator
        .quote(Provider::Titan, &request)
        .await
        .expect("titan quote should succeed");

    assert_eq!(quote.provider, Provider::Titan);
    assert!(quote.output_amount > 0, "output_amount must be > 0");
    assert_eq!(quote.input_mint, env.input_mint);
    assert_eq!(quote.output_mint, env.output_mint);

    let swap_result = aggregator
        .swap(&quote, &pubkey, &rpc_client)
        .await
        .expect("titan swap should succeed");

    assert_swap_result_valid(swap_result, &pubkey);
}

#[tokio::test]
#[ignore = "requires env vars and real API access"]
async fn test_dflow_quote_and_swap() {
    let env = load_test_env();
    let aggregator = SwapAggregator::new(build_swap_config(&env));
    let request = build_quote_request(&env);
    let rpc_client = RpcClient::new(env.rpc_url.clone());
    let pubkey = env.keypair.pubkey();

    let quote = aggregator
        .quote(Provider::Dflow, &request)
        .await
        .expect("dflow quote should succeed");

    assert_eq!(quote.provider, Provider::Dflow);
    assert!(quote.output_amount > 0, "output_amount must be > 0");
    assert_eq!(quote.input_mint, env.input_mint);
    assert_eq!(quote.output_mint, env.output_mint);

    let swap_result = aggregator
        .swap(&quote, &pubkey, &rpc_client)
        .await
        .expect("dflow swap should succeed");

    assert_swap_result_valid(swap_result, &pubkey);
}
