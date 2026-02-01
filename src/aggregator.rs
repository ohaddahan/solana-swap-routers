use std::pin::Pin;

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

use crate::{
    error::SwapError,
    types::{Provider, QuoteRequest, QuoteResponse, SwapConfig, SwapResult},
};

type QuoteFuture<'a> =
    Pin<Box<dyn std::future::Future<Output = Result<QuoteResponse, SwapError>> + Send + 'a>>;

#[cfg(feature = "dflow")]
use crate::dflow::DflowProvider;
#[cfg(feature = "jupiter")]
use crate::jupiter::JupiterProvider;
#[cfg(feature = "titan")]
use crate::titan::TitanProvider;

pub struct SwapAggregator {
    pub default_slippage_bps: u16,
    #[cfg(feature = "jupiter")]
    pub jupiter: Option<JupiterProvider>,
    #[cfg(feature = "titan")]
    pub titan: Option<TitanProvider>,
    #[cfg(feature = "dflow")]
    pub dflow: Option<DflowProvider>,
}

impl SwapAggregator {
    pub fn new(config: SwapConfig) -> Self {
        let SwapConfig {
            default_slippage_bps,
            jupiter_api_url,
            jupiter_api_key,
            titan_ws_url,
            titan_token,
            dflow_api_url,
            dflow_api_key,
            dflow_max_route_length,
        } = config;

        #[cfg(not(feature = "jupiter"))]
        let _ = (jupiter_api_url, jupiter_api_key);
        #[cfg(not(feature = "titan"))]
        let _ = (titan_ws_url, titan_token);
        #[cfg(not(feature = "dflow"))]
        let _ = (dflow_api_url, dflow_api_key, dflow_max_route_length);

        Self {
            default_slippage_bps,
            #[cfg(feature = "jupiter")]
            jupiter: Some(JupiterProvider::new(jupiter_api_url, jupiter_api_key)),
            #[cfg(feature = "titan")]
            titan: Some(TitanProvider::new(titan_ws_url, titan_token)),
            #[cfg(feature = "dflow")]
            dflow: Some(DflowProvider::new(dflow_api_url, dflow_api_key, dflow_max_route_length)),
        }
    }

    pub async fn quote(
        &self,
        provider: Provider,
        request: &QuoteRequest,
    ) -> Result<QuoteResponse, SwapError> {
        match provider {
            Provider::Jupiter => {
                #[cfg(feature = "jupiter")]
                {
                    let p = self
                        .jupiter
                        .as_ref()
                        .ok_or(SwapError::ProviderNotConfigured(Provider::Jupiter))?;
                    p.quote(request, self.default_slippage_bps).await
                }
                #[cfg(not(feature = "jupiter"))]
                {
                    Err(SwapError::ProviderNotConfigured(Provider::Jupiter))
                }
            }
            Provider::Titan => {
                #[cfg(feature = "titan")]
                {
                    let p = self
                        .titan
                        .as_ref()
                        .ok_or(SwapError::ProviderNotConfigured(Provider::Titan))?;
                    p.quote(request, self.default_slippage_bps).await
                }
                #[cfg(not(feature = "titan"))]
                {
                    Err(SwapError::ProviderNotConfigured(Provider::Titan))
                }
            }
            Provider::Dflow => {
                #[cfg(feature = "dflow")]
                {
                    let p = self
                        .dflow
                        .as_ref()
                        .ok_or(SwapError::ProviderNotConfigured(Provider::Dflow))?;
                    p.quote(request, self.default_slippage_bps).await
                }
                #[cfg(not(feature = "dflow"))]
                {
                    Err(SwapError::ProviderNotConfigured(Provider::Dflow))
                }
            }
        }
    }

    pub async fn quote_all(&self, request: &QuoteRequest) -> Vec<Result<QuoteResponse, SwapError>> {
        let mut futures: Vec<QuoteFuture<'_>> = Vec::new();

        #[cfg(feature = "jupiter")]
        if let Some(ref p) = self.jupiter {
            futures.push(Box::pin(p.quote(request, self.default_slippage_bps)));
        }

        #[cfg(feature = "titan")]
        if let Some(ref p) = self.titan {
            futures.push(Box::pin(p.quote(request, self.default_slippage_bps)));
        }

        #[cfg(feature = "dflow")]
        if let Some(ref p) = self.dflow {
            futures.push(Box::pin(p.quote(request, self.default_slippage_bps)));
        }

        futures::future::join_all(futures).await
    }

    pub async fn swap(
        &self,
        quote: &QuoteResponse,
        user_pubkey: &Pubkey,
        _rpc_client: &RpcClient,
    ) -> Result<SwapResult, SwapError> {
        match quote.provider {
            Provider::Jupiter => {
                #[cfg(feature = "jupiter")]
                {
                    let p = self
                        .jupiter
                        .as_ref()
                        .ok_or(SwapError::ProviderNotConfigured(Provider::Jupiter))?;
                    p.swap(quote, user_pubkey, _rpc_client).await
                }
                #[cfg(not(feature = "jupiter"))]
                {
                    Err(SwapError::ProviderNotConfigured(Provider::Jupiter))
                }
            }
            Provider::Titan => {
                #[cfg(feature = "titan")]
                {
                    let p = self
                        .titan
                        .as_ref()
                        .ok_or(SwapError::ProviderNotConfigured(Provider::Titan))?;
                    p.swap(quote, user_pubkey, _rpc_client).await
                }
                #[cfg(not(feature = "titan"))]
                {
                    Err(SwapError::ProviderNotConfigured(Provider::Titan))
                }
            }
            Provider::Dflow => {
                #[cfg(feature = "dflow")]
                {
                    let p = self
                        .dflow
                        .as_ref()
                        .ok_or(SwapError::ProviderNotConfigured(Provider::Dflow))?;
                    p.swap(quote, user_pubkey).await
                }
                #[cfg(not(feature = "dflow"))]
                {
                    Err(SwapError::ProviderNotConfigured(Provider::Dflow))
                }
            }
        }
    }
}
