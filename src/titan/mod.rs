pub mod types;

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::OnceCell;
use tracing::debug;

use titan_rust_client::{
    types::{SwapMode, SwapParams, SwapPriceRequest, SwapQuoteRequest, TransactionParams},
    TitanClient, TitanConfig, TitanInstructions,
};

use crate::{
    error::SwapError,
    types::{Provider, QuoteRequest, QuoteResponse, SwapResult},
};

use self::types::select_best_route;

const DEFAULT_TITAN_WS_URL: &str = "wss://api.titan.ag/api/v1/ws";
const TITAN_WS_URL_ENV: &str = "TITAN_WS_URL";

pub struct TitanProvider {
    pub ws_url: String,
    pub token: String,
    client: OnceCell<TitanClient>,
}

impl TitanProvider {
    pub fn new(ws_url: Option<String>, token: Option<String>) -> Self {
        Self {
            ws_url: ws_url
                .or_else(|| std::env::var(TITAN_WS_URL_ENV).ok())
                .unwrap_or_else(|| DEFAULT_TITAN_WS_URL.to_string()),
            token: token.unwrap_or_default(),
            client: OnceCell::new(),
        }
    }

    async fn get_client(&self) -> Result<&TitanClient, SwapError> {
        self.client
            .get_or_try_init(|| async {
                let config = TitanConfig::new(&self.ws_url, &self.token);
                TitanClient::new(config)
                    .await
                    .map_err(|e| SwapError::Titan(e.to_string()))
            })
            .await
    }

    pub async fn quote(
        &self,
        request: &QuoteRequest,
        default_slippage_bps: u16,
    ) -> Result<QuoteResponse, SwapError> {
        let client = self.get_client().await?;

        let price_request = SwapPriceRequest {
            input_mint: request.input_mint.to_bytes().into(),
            output_mint: request.output_mint.to_bytes().into(),
            amount: request.amount,
            ..Default::default()
        };

        debug!("titan get_swap_price");
        let price = client
            .get_swap_price(price_request)
            .await
            .map_err(|e| SwapError::Titan(e.to_string()))?;

        let slippage_bps = request.slippage_bps.unwrap_or(default_slippage_bps);

        let provider_data = serde_json::json!({
            "inputMint": request.input_mint.to_string(),
            "outputMint": request.output_mint.to_string(),
            "amount": request.amount,
            "slippageBps": slippage_bps,
            "onlyDirectRoutes": request.only_direct_routes,
        });

        Ok(QuoteResponse {
            provider: Provider::Titan,
            input_mint: request.input_mint,
            output_mint: request.output_mint,
            input_amount: price.amount_in,
            output_amount: price.amount_out,
            price_impact_bps: None,
            slippage_bps,
            provider_data,
        })
    }

    pub async fn swap(
        &self,
        quote: &QuoteResponse,
        user_pubkey: &Pubkey,
        rpc_client: &RpcClient,
    ) -> Result<SwapResult, SwapError> {
        let client = self.get_client().await?;

        let amount = quote.provider_data["amount"]
            .as_u64()
            .ok_or_else(|| SwapError::Titan("missing amount in provider_data".to_string()))?;
        let slippage_bps = quote.provider_data["slippageBps"]
            .as_u64()
            .map(|v| v as u16);

        let only_direct_routes = quote.provider_data["onlyDirectRoutes"].as_bool();

        let swap_request = SwapQuoteRequest {
            swap: SwapParams {
                input_mint: quote.input_mint.to_bytes().into(),
                output_mint: quote.output_mint.to_bytes().into(),
                amount,
                swap_mode: Some(SwapMode::ExactIn),
                slippage_bps,
                only_direct_routes,
                ..Default::default()
            },
            transaction: TransactionParams {
                user_public_key: user_pubkey.to_bytes().into(),
                ..Default::default()
            },
            ..Default::default()
        };

        debug!("titan new_swap_quote_stream");
        let mut stream = client
            .new_swap_quote_stream(swap_request)
            .await
            .map_err(|e| SwapError::Titan(e.to_string()))?;

        let quotes = stream.recv().await.ok_or(SwapError::NoRouteFound)?;

        let _ = stream.stop().await;

        let all_routes: Vec<_> = quotes.quotes.into_values().collect();
        let route = select_best_route(all_routes).ok_or(SwapError::NoRouteFound)?;

        let output = TitanInstructions::prepare_instructions(&route, rpc_client)
            .await
            .map_err(|e| SwapError::Titan(e.to_string()))?;

        Ok(SwapResult::Instructions {
            instructions: output.instructions,
            address_lookup_tables: output.address_lookup_table_accounts,
            compute_units: output.compute_units.map(|cu| cu as u32),
        })
    }
}
