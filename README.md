# solana-swap

Unified Rust crate for executing token swaps on Solana through multiple DEX aggregators — Jupiter, Titan, and Dflow — behind a single API.

## Overview

```
QuoteRequest ──► SwapAggregator ──► QuoteResponse ──► SwapResult ──► VersionedTransaction
                  │   │   │
                  ▼   ▼   ▼
              Jupiter Titan Dflow
              (REST)  (WS)  (REST)
```

Ask any provider for a quote, get back a normalized `QuoteResponse`, then build swap instructions or a ready-to-sign transaction. Each provider is behind a Cargo feature flag — compile only what you need.

## Usage

```rust
use solana_swap::{SwapAggregator, SwapConfig, QuoteRequest, Provider};
use solana_sdk::pubkey::Pubkey;

let aggregator = SwapAggregator::new(SwapConfig {
    default_slippage_bps: 300,
    jupiter_api_url: None,  // falls back to JUPITER_API_URL env or built-in default
    jupiter_api_key: Some("your-key".into()),
    titan_ws_url: None,
    titan_token: None,
    dflow_api_url: None,
    dflow_api_key: None,
    dflow_max_route_length: None,
});

let request = QuoteRequest {
    input_mint: sol_mint,
    output_mint: usdc_mint,
    amount: 1_000_000,
    slippage_bps: Some(300),
    only_direct_routes: None, // None = allow multi-hop, Some(true) = direct only
};

// Quote from a specific provider
let quote = aggregator.quote(Provider::Jupiter, &request).await?;

// Or quote all providers concurrently
let quotes = aggregator.quote_all(&request).await;

// Build swap instructions
let swap_result = aggregator.swap(&quote, &user_pubkey, &rpc_client).await?;

// Normalize to an unsigned transaction
let unsigned_tx = swap_result.into_unsigned_transaction(&payer, blockhash)?;
```

## Providers

| Provider | Protocol | Quote | Swap Result |
|----------|----------|-------|-------------|
| **Jupiter** | REST | `GET /quote` | Instructions + ALTs |
| **Titan** | WebSocket | Streaming price → quote stream | Instructions + ALTs |
| **Dflow** | REST | `GET /order` (no pubkey) | Pre-built transaction |

## Features

```toml
[dependencies]
solana-swap = "0.1"                                    # all providers (default)
solana-swap = { version = "0.1", features = ["jupiter"] }   # jupiter only
solana-swap = { version = "0.1", features = ["dflow"] }     # dflow only
```

Default features: `jupiter`, `titan`, `dflow`.

## Configuration

`SwapConfig` fields resolve in order: explicit value > environment variable > compiled default.

| Field | Env Var | Default |
|-------|---------|---------|
| `jupiter_api_url` | `JUPITER_API_URL` | `https://lite-api.jup.ag/swap/v1` |
| `jupiter_api_key` | — | None |
| `titan_ws_url` | `TITAN_WS_URL` | `wss://api.titan.ag/api/v1/ws` |
| `titan_token` | — | None |
| `dflow_api_url` | `DFLOW_API_URL` | `https://dev-quote-api.dflow.net` |
| `dflow_api_key` | — | None |
| `dflow_max_route_length` | — | None |

## Routing Options

**`only_direct_routes`** — When `Some(true)`, restricts to single-hop routes (input mint directly to output mint). Simpler transactions and lower slippage risk, but potentially worse pricing.

**`dflow_max_route_length`** — Dflow-specific: limits the number of hops without forcing single-hop. For example, `Some(2)` allows up to 2-hop routes.

## Building

```bash
cargo build                                          # all features
cargo build --no-default-features --features jupiter # jupiter only
cargo clippy -- -D warnings
cargo fmt --check
```

## Testing

Integration tests hit real APIs and require env vars. All tests are `#[ignore]`'d.

```bash
# Set up env (or use .env file)
export TEST_INPUT_MINT="So11111111111111111111111111111111111111112"
export TEST_OUTPUT_MINT="EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
export TEST_KEYPAIR_PATH="~/.config/solana/id.json"
export TEST_RPC_URL="https://api.mainnet-beta.solana.com"

# Run with nextest (preferred)
cargo nextest run --test main --run-ignored ignored-only

# Or with cargo test
cargo test --test main -- --ignored --nocapture
```

Optional env vars: `TEST_AMOUNT`, `TEST_SLIPPAGE_BPS`, `TEST_SEND_TX` (set to `1` to actually send transactions), `TEST_JUPITER_API_KEY`, `TEST_TITAN_WS_URL`, `TEST_TITAN_TOKEN`, `TEST_DFLOW_API_KEY`.

## Project Structure

```
src/
├── lib.rs              # Public re-exports
├── aggregator.rs       # SwapAggregator (dispatch + quote_all)
├── types.rs            # QuoteRequest, QuoteResponse, SwapResult, SwapConfig
├── error.rs            # SwapError enum
├── jupiter/            # REST provider
├── titan/              # WebSocket provider
└── dflow/              # REST provider (combined quote+swap endpoint)

tests/
├── main.rs             # Single test binary entry point
├── common/mod.rs       # Shared helpers
├── jupiter/            # Jupiter test variants
├── titan/              # Titan test variants
└── dflow/              # Dflow test variants (incl. max_route_length)
```

## License

Private — not published to crates.io.
