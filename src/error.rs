use crate::types::Provider;

#[derive(Debug, thiserror::Error)]
pub enum SwapError {
    #[error("no route found")]
    NoRouteFound,

    #[error("insufficient liquidity")]
    InsufficientLiquidity,

    #[error("quote expired")]
    QuoteExpired,

    #[error("provider not configured: {0}")]
    ProviderNotConfigured(Provider),

    #[error("{provider} API error: {message}")]
    Api { provider: Provider, message: String },

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("solana error: {0}")]
    Solana(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[cfg(feature = "titan")]
    #[error("titan error: {0}")]
    Titan(String),
}
