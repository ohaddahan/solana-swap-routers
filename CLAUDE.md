# solana-swap

Unified Rust crate wrapping Jupiter (REST), Titan (WebSocket), and Dflow (REST) swap aggregators behind a common API.

## Architecture

- `SwapAggregator` dispatches to providers via `Provider` enum (no traits)
- Each provider is feature-gated: `jupiter`, `titan`, `dflow` (all default)
- `QuoteResponse.provider_data` carries opaque JSON for the swap step
- `SwapResult` has two variants: `Instructions` (Jupiter/Titan) and `Transaction` (Dflow)

## Key Files

- `src/aggregator.rs` - SwapAggregator with quote/quote_all/swap
- `src/types.rs` - QuoteRequest, QuoteResponse, SwapResult, SwapConfig, Provider
- `src/error.rs` - SwapError enum
- `src/jupiter/` - REST: GET /quote, POST /swap-instructions
- `src/titan/` - WebSocket via titan-rust-client, lazy OnceCell connect
- `src/dflow/` - REST: GET /order endpoint (quote + swap combined)

## Build

```
cargo build                                          # all features
cargo build --no-default-features --features jupiter # jupiter only
cargo build --no-default-features --features titan   # titan only
cargo build --no-default-features --features dflow   # dflow only
cargo clippy -- -D warnings
cargo fmt --check
```

## Integration Tests

- `tests/swap.rs` — quote+swap through each provider using real APIs
- All tests are `#[ignore]`'d; run with `cargo test --test swap -- --ignored --nocapture`
- Config via env vars: `TEST_INPUT_MINT`, `TEST_OUTPUT_MINT`, `TEST_KEYPAIR_PATH`, `TEST_RPC_URL` (required); `TEST_AMOUNT`, `TEST_SLIPPAGE_BPS`, `TEST_JUPITER_API_KEY`, `TEST_TITAN_WS_URL`, `TEST_TITAN_TOKEN`, `TEST_DFLOW_API_KEY` (optional)
- Tests do NOT sign or send transactions — they stop after `into_unsigned_transaction()`

## Gotchas

- `solana-address-lookup-table-interface` exports `AddressLookupTable` under `state::` submodule, not root
- `AddressLookupTable::deserialize()` returns `InstructionError` — needs explicit type annotation in `.map_err()`
- When destructuring `SwapConfig` in `new()`, unused feature-gated fields need `let _ = (field1, field2)` under `#[cfg(not(feature = "..."))]` to avoid warnings
- Clippy lint name is `uninlined_format_args` not `unlined_format_args`
- `SwapAggregator::swap()` parameter `rpc_client` must be `_rpc_client` because Dflow doesn't use it
- Jupiter instruction data comes base64-encoded from the API
- Dflow transaction comes base64-encoded, deserialized with bincode
- Titan `OnceCell` lazy init means first call pays connection cost
- Integration tests (separate binary) inherit crate-level clippy denies — need `#![allow(..., reason = "...")]` at file top for `unwrap_used`/`expect_used`/`panic`
- `#[ignore]` requires a reason string (`#[ignore = "reason"]`) due to `clippy::ignore_without_reason`
- `Keypair::from_bytes` is deprecated — use `Keypair::try_from(slice)` instead

## Dependencies

- titan-rust-client at `../titan-rust-client` (path dep, optional)
- Jupiter and Dflow use reqwest REST calls
- `futures` crate for `join_all` in `quote_all`
