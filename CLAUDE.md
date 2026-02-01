# solana-swap

Unified Rust crate wrapping Jupiter (REST), Titan (WebSocket), and Dflow (REST) swap aggregators behind a common API.

## Architecture

- `SwapAggregator` dispatches to providers via `Provider` enum (no traits)
- Each provider is feature-gated: `jupiter`, `titan`, `dflow` (all default)
- `QuoteResponse.provider_data` carries opaque JSON for the swap step (amounts, slippage, `only_direct_routes`)
- `SwapResult` has two variants: `Instructions` (Jupiter/Titan) and `Transaction` (Dflow)
- `QuoteRequest.only_direct_routes` controls whether multi-hop routes are allowed (None = provider default)

## Key Files

### Library
- `src/aggregator.rs` - SwapAggregator with quote/quote_all/swap
- `src/types.rs` - QuoteRequest (with `only_direct_routes`), QuoteResponse, SwapResult, SwapConfig, Provider
- `src/error.rs` - SwapError enum
- `src/jupiter/` - REST: GET /quote (`onlyDirectRoutes` query param), POST /swap-instructions
- `src/titan/` - WebSocket via titan-rust-client, lazy OnceCell connect
- `src/dflow/` - REST: GET /order endpoint (quote + swap combined)

### Tests (single binary, DCA module pattern)
- `tests/main.rs` - entry point, mod declarations, clippy allows
- `tests/common/mod.rs` - TestEnv, helpers (load_test_env, build_swap_config, build_quote_request, finalize_swap, print_quote)
- `tests/jupiter/{quote_and_swap,quote_and_swap_direct}.rs`
- `tests/titan/{quote_and_swap,quote_and_swap_direct}.rs`
- `tests/dflow/{quote_and_swap,quote_and_swap_direct,max_route_length}.rs`

### Config
- `.config/nextest.toml` - nextest profile (success-output=immediate, slow-timeout=60s)

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

- Single binary at `tests/main.rs` using DCA module pattern: `tests/{common,jupiter,titan,dflow}/`
- Prefer nextest: `cargo nextest run --test main --run-ignored ignored-only`
- Fallback: `cargo test --test main -- --ignored --nocapture`
- List tests: `cargo nextest list --test main --run-ignored ignored-only`
- Nextest config in `.config/nextest.toml` — `success-output = "immediate"` so test stdout (quote details, tx signatures) shows during runs
- Config via env vars (or `.env` file via dotenvy): `TEST_INPUT_MINT`, `TEST_OUTPUT_MINT`, `TEST_KEYPAIR_PATH`, `TEST_RPC_URL` (required); `TEST_AMOUNT`, `TEST_SLIPPAGE_BPS`, `TEST_SEND_TX`, `TEST_JUPITER_API_KEY`, `TEST_TITAN_WS_URL`, `TEST_TITAN_TOKEN`, `TEST_DFLOW_API_KEY` (optional)
- By default tests do NOT sign or send — set `TEST_SEND_TX=1` to actually sign, send, and print tx signatures

### Test Matrix (7 tests)
| Provider | Test | `only_direct_routes` | `max_route_length` |
|----------|------|---------------------|--------------------|
| Jupiter | quote_and_swap | None | — |
| Jupiter | quote_and_swap_direct | Some(true) | — |
| Titan | quote_and_swap | None | — |
| Titan | quote_and_swap_direct | Some(true) | — |
| Dflow | quote_and_swap | None | None |
| Dflow | quote_and_swap_direct | Some(true) | None |
| Dflow | max_route_length | None | Some(1) |

## Gotchas

- `solana-address-lookup-table-interface` exports `AddressLookupTable` under `state::` submodule, not root
- `AddressLookupTable::deserialize()` returns `InstructionError` — needs explicit type annotation in `.map_err()`
- When destructuring `SwapConfig` in `new()`, unused feature-gated fields need `let _ = (field1, field2)` under `#[cfg(not(feature = "..."))]` to avoid warnings
- Clippy lint name is `uninlined_format_args` not `unlined_format_args`
- `SwapAggregator::swap()` parameter `rpc_client` must be `_rpc_client` because Dflow doesn't use it
- Jupiter instruction data comes base64-encoded from the API
- Dflow transaction comes base64-encoded, deserialized with bincode
- Titan `OnceCell` lazy init means first call pays connection cost
- Provider URLs resolve in order: config value → env var → compiled default. Env vars: `JUPITER_API_URL`, `TITAN_WS_URL`, `DFLOW_API_URL`
- Dflow default URL is `https://dev-quote-api.dflow.net` (dev endpoint)
- Integration tests (separate binary) inherit crate-level clippy denies — need `#![allow(..., reason = "...")]` at file top for `unwrap_used`/`expect_used`/`panic`
- `#[ignore]` requires a reason string (`#[ignore = "reason"]`) due to `clippy::ignore_without_reason`
- `Keypair::from_bytes` is deprecated — use `Keypair::try_from(slice)` instead
- rustls 0.23+ needs an explicit crypto provider — `SwapAggregator::new()` calls `rustls::crypto::ring::default_provider().install_default()` (idempotent, ignores if already installed)
- `VersionedTransaction::try_new()` fails with "not enough signers" if given fewer keypairs than the message requires — for unsigned transactions, manually construct with `vec![Signature::default(); num_signers]` placeholder signatures instead
- Dflow `onlyDirectRoutes` and `maxRouteLength` are separate API params — explicit `only_direct_routes` from `QuoteRequest` takes priority; `maxRouteLength` without explicit `only_direct_routes` defaults to `onlyDirectRoutes=false`
- `QuoteRequest.only_direct_routes` flows through `provider_data` JSON for Titan and Dflow (needed across quote→swap boundary), and as a query param for Jupiter

## Dependencies

- titan-rust-client at `../titan-rust-client` (path dep, optional)
- Jupiter and Dflow use reqwest REST calls
- `futures` crate for `join_all` in `quote_all`
