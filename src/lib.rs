pub mod aggregator;
pub mod error;
pub mod types;

#[cfg(feature = "dflow")]
pub mod dflow;
#[cfg(feature = "jupiter")]
pub mod jupiter;
#[cfg(feature = "titan")]
pub mod titan;

pub use aggregator::SwapAggregator;
pub use error::SwapError;
pub use types::{
    CpiSwapResult, Provider, QuoteRequest, QuoteResponse, SwapConfig, SwapMode, SwapResult,
    JUPITER_PROGRAM, TITAN_PROGRAM,
};
