pub mod types;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use solana_sdk::{pubkey::Pubkey, transaction::VersionedTransaction};
use tracing::debug;

use crate::{
    error::SwapError,
    types::{Provider, QuoteRequest, QuoteResponse, SwapResult},
};

use self::types::DflowOrderResponse;

const DEFAULT_DFLOW_API_URL: &str = "https://quote-api.dflow.net";

pub struct DflowProvider {
    pub client: reqwest::Client,
    pub base_url: String,
    pub api_key: Option<String>,
    pub max_route_length: Option<u32>,
}

impl DflowProvider {
    pub fn new(base_url: Option<String>, api_key: Option<String>, max_route_length: Option<u32>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.unwrap_or_else(|| DEFAULT_DFLOW_API_URL.to_string()),
            api_key,
            max_route_length,
        }
    }

    pub async fn quote(
        &self,
        request: &QuoteRequest,
        default_slippage_bps: u16,
    ) -> Result<QuoteResponse, SwapError> {
        let response = self
            .fetch_order(request, default_slippage_bps, None)
            .await?;

        let in_amount: u64 = response
            .in_amount
            .parse()
            .map_err(|e: std::num::ParseIntError| SwapError::Serialization(e.to_string()))?;
        let out_amount: u64 = response
            .out_amount
            .parse()
            .map_err(|e: std::num::ParseIntError| SwapError::Serialization(e.to_string()))?;

        let price_impact_bps = response
            .price_impact_pct
            .and_then(|pct| pct.parse::<f64>().ok().map(|p| (p * 100.0) as u16));

        let provider_data = serde_json::json!({
            "inputMint": request.input_mint.to_string(),
            "outputMint": request.output_mint.to_string(),
            "amount": request.amount,
            "slippageBps": request.slippage_bps.unwrap_or(default_slippage_bps),
        });

        Ok(QuoteResponse {
            provider: Provider::Dflow,
            input_mint: request.input_mint,
            output_mint: request.output_mint,
            input_amount: in_amount,
            output_amount: out_amount,
            price_impact_bps,
            slippage_bps: response.slippage_bps,
            provider_data,
        })
    }

    pub async fn swap(
        &self,
        quote: &QuoteResponse,
        user_pubkey: &Pubkey,
    ) -> Result<SwapResult, SwapError> {
        let amount: u64 = quote.provider_data["amount"].as_u64().ok_or_else(|| {
            SwapError::Serialization("missing amount in provider_data".to_string())
        })?;
        let slippage_bps: u16 = quote.provider_data["slippageBps"].as_u64().ok_or_else(|| {
            SwapError::Serialization("missing slippageBps in provider_data".to_string())
        })? as u16;

        let request = QuoteRequest {
            input_mint: quote.input_mint,
            output_mint: quote.output_mint,
            amount,
            slippage_bps: Some(slippage_bps),
        };

        let response = self
            .fetch_order(&request, slippage_bps, Some(user_pubkey))
            .await?;

        let tx_base64 = response.transaction.ok_or_else(|| SwapError::Api {
            provider: Provider::Dflow,
            message: "no transaction in order response".to_string(),
        })?;

        let tx_bytes = BASE64
            .decode(&tx_base64)
            .map_err(|e| SwapError::Serialization(e.to_string()))?;

        let transaction: VersionedTransaction =
            bincode::deserialize(&tx_bytes).map_err(|e| SwapError::Serialization(e.to_string()))?;

        let last_valid_block_height = response.last_valid_block_height.unwrap_or(0);

        Ok(SwapResult::Transaction {
            transaction,
            last_valid_block_height,
        })
    }

    async fn fetch_order(
        &self,
        request: &QuoteRequest,
        default_slippage_bps: u16,
        user_pubkey: Option<&Pubkey>,
    ) -> Result<DflowOrderResponse, SwapError> {
        let url = format!("{}/order", self.base_url);

        let mut query: Vec<(&str, String)> = vec![
            ("inputMint", request.input_mint.to_string()),
            ("outputMint", request.output_mint.to_string()),
            ("amount", request.amount.to_string()),
            (
                "slippageBps",
                request
                    .slippage_bps
                    .unwrap_or(default_slippage_bps)
                    .to_string(),
            ),
        ];

        if let Some(pk) = user_pubkey {
            query.push(("userPublicKey", pk.to_string()));
        }

        if let Some(max_legs) = self.max_route_length {
            query.push(("maxRouteLength", max_legs.to_string()));
            query.push(("onlyDirectRoutes", "false".to_string()));
        }

        let mut req = self.client.get(&url).query(&query);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }

        debug!("dflow order: {url}");
        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if body.contains("route_not_found") || body.contains("No route") {
                return Err(SwapError::NoRouteFound);
            }
            return Err(SwapError::Api {
                provider: Provider::Dflow,
                message: format!("HTTP {status}: {body}"),
            });
        }

        response
            .json::<DflowOrderResponse>()
            .await
            .map_err(|e| SwapError::Serialization(e.to_string()))
    }
}
