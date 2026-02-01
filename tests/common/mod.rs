use std::str::FromStr;

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    hash::Hash, pubkey::Pubkey, signature::Keypair, signer::Signer,
    transaction::VersionedTransaction,
};

use solana_swap::{QuoteRequest, QuoteResponse, SwapConfig, SwapResult};

pub struct TestEnv {
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub amount: u64,
    pub slippage_bps: u16,
    pub keypair: Keypair,
    pub rpc_url: String,
    pub send_tx: bool,
    pub jupiter_api_key: Option<String>,
    pub titan_ws_url: Option<String>,
    pub titan_token: Option<String>,
    pub dflow_api_key: Option<String>,
}

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("env var {name} is required but not set"))
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

pub fn load_test_env() -> TestEnv {
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
    let send_tx = optional_env("TEST_SEND_TX").is_some_and(|v| v == "1" || v == "true");

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
        send_tx,
        jupiter_api_key: optional_env("TEST_JUPITER_API_KEY"),
        titan_ws_url: optional_env("TEST_TITAN_WS_URL"),
        titan_token: optional_env("TEST_TITAN_TOKEN"),
        dflow_api_key: optional_env("TEST_DFLOW_API_KEY"),
    }
}

pub fn build_swap_config(env: &TestEnv, dflow_max_route_length: Option<u32>) -> SwapConfig {
    SwapConfig {
        default_slippage_bps: env.slippage_bps,
        jupiter_api_url: None,
        jupiter_api_key: env.jupiter_api_key.clone(),
        titan_ws_url: env.titan_ws_url.clone(),
        titan_token: env.titan_token.clone(),
        dflow_api_url: None,
        dflow_api_key: env.dflow_api_key.clone(),
        dflow_max_route_length,
    }
}

pub fn build_quote_request(env: &TestEnv, only_direct_routes: Option<bool>) -> QuoteRequest {
    QuoteRequest {
        input_mint: env.input_mint,
        output_mint: env.output_mint,
        amount: env.amount,
        slippage_bps: Some(env.slippage_bps),
        only_direct_routes,
    }
}

fn short_pubkey(pk: &Pubkey) -> String {
    let s = pk.to_string();
    if s.len() > 8 {
        format!("{}..{}", &s[..4], &s[s.len() - 4..])
    } else {
        s
    }
}

pub fn print_quote(test_name: &str, quote: &QuoteResponse) {
    let bar = "━".repeat(56);
    println!("\n{bar}");
    println!("  {test_name}  [{provider}]", provider = quote.provider,);
    println!("{bar}");
    println!(
        "  {in_amount} {in_mint}  →  {out_amount} {out_mint}",
        in_amount = quote.input_amount,
        in_mint = short_pubkey(&quote.input_mint),
        out_amount = quote.output_amount,
        out_mint = short_pubkey(&quote.output_mint),
    );
    let impact = quote
        .price_impact_bps
        .map(|b| format!(" · impact: {b} bps"))
        .unwrap_or_default();
    println!("  slippage: {} bps{impact}", quote.slippage_bps);
}

pub async fn finalize_swap(
    test_name: &str,
    result: SwapResult,
    keypair: &Keypair,
    rpc_client: &RpcClient,
    send: bool,
) {
    let payer = keypair.pubkey();

    match &result {
        SwapResult::Instructions {
            instructions,
            compute_units,
            ..
        } => {
            assert!(
                !instructions.is_empty(),
                "expected at least one instruction"
            );
            let cu = compute_units.map_or("n/a".to_string(), |c| c.to_string());
            println!("  swap: {} instructions · CU: {cu}", instructions.len());
        }
        SwapResult::Transaction { transaction, .. } => {
            assert!(
                !transaction.message.instructions().is_empty(),
                "expected at least one instruction in transaction"
            );
            println!(
                "  swap: transaction ({} instructions)",
                transaction.message.instructions().len()
            );
        }
    }

    if send {
        let blockhash = rpc_client
            .get_latest_blockhash()
            .await
            .expect("failed to get latest blockhash");

        let unsigned_tx = result
            .into_unsigned_transaction(&payer, blockhash)
            .expect("into_unsigned_transaction should succeed");

        let signed_tx = VersionedTransaction::try_new(unsigned_tx.message, &[keypair])
            .expect("signing should succeed");

        let sig = rpc_client
            .send_and_confirm_transaction(&signed_tx)
            .await
            .expect("send_and_confirm_transaction should succeed");

        println!("  tx: {sig}");
    } else {
        let tx = result
            .into_unsigned_transaction(&payer, Hash::default())
            .expect("into_unsigned_transaction should succeed");

        assert!(
            !tx.message.instructions().is_empty(),
            "unsigned transaction should have instructions"
        );
        println!("  tx: dry run (not sent)");
    }

    let bar = "━".repeat(56);
    println!("{bar}\n");
    println!("  {test_name}: OK ✓\n");
}
