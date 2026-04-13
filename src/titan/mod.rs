use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::OnceCell;
use tracing::debug;

use titan_rust_client::{
    types::SwapPriceRequest,
    TitanClient, TitanConfig,
};

use crate::{
    error::SwapError,
    types::{Provider, QuoteRequest, QuoteResponse, SwapResult},
};

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

        let slippage_bps = request.slippage_bps.unwrap_or(default_slippage_bps);

        let dexes = request.dexes.as_ref().map(|d| {
            d.split(',')
                .map(|s| s.trim().to_string())
                .collect::<Vec<_>>()
        });

        let exclude_dexes = request.exclude_dexes.as_ref().map(|d| {
            d.split(',')
                .map(|s| s.trim().to_string())
                .collect::<Vec<_>>()
        });

        let price_request = SwapPriceRequest {
            input_mint: request.input_mint.to_bytes().into(),
            output_mint: request.output_mint.to_bytes().into(),
            amount: request.amount,
            dexes,
            exclude_dexes,
        };

        debug!("titan get_swap_price (quote)");
        let price = client
            .get_swap_price(price_request)
            .await
            .map_err(|e| SwapError::Titan(e.to_string()))?;

        if price.amount_out == 0 {
            return Err(SwapError::NoRouteFound);
        }

        let provider_data = serde_json::to_value(&price)
            .unwrap_or_else(|_| serde_json::json!({"error": "serialization_failed"}));

        Ok(QuoteResponse {
            provider: Provider::Titan,
            input_mint: request.input_mint,
            output_mint: request.output_mint,
            input_amount: request.amount,
            output_amount: price.amount_out,
            price_impact_bps: None,
            slippage_bps,
            provider_data,
        })
    }

    pub async fn swap(
        &self,
        _quote: &QuoteResponse,
        _user_pubkey: &Pubkey,
        _rpc_client: &RpcClient,
    ) -> Result<SwapResult, SwapError> {
        Err(SwapError::Titan(
            "Titan get_swap_price does not provide swap instructions".to_string(),
        ))
    }
}
