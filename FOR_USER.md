# solana-swap — Understanding the Crate

## What This Crate Does

This is a unified Rust library for executing token swaps on Solana through multiple DEX aggregators. Think of it as a "swap router" — you give it two tokens and an amount, and it talks to Jupiter, Titan, or Dflow behind the scenes to find and execute the best swap route.

## Why Three Providers?

Different aggregators have different strengths:

- **Jupiter** is the most popular Solana DEX aggregator with the widest route coverage. It uses a traditional REST API: you ask for a quote, then ask for swap instructions.
- **Titan** uses WebSockets for real-time streaming quotes. It can continuously update quotes as market conditions change, which is useful for time-sensitive operations.
- **Dflow** combines quoting and transaction building into a single `/order` endpoint. When you provide a user wallet address, it returns a ready-to-sign transaction directly — no instruction assembly needed.

By wrapping all three behind a common interface, downstream code (like the worker-service) can easily switch between them or compare quotes from all providers simultaneously.

## Technical Architecture

```
         QuoteRequest { input_mint, output_mint, amount, slippage, only_direct_routes }
                                        │
                                        ▼
┌──────────────────────────────────────────────────────────────┐
│                      SwapAggregator                           │
│                                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │   Jupiter     │  │    Titan     │  │    Dflow     │        │
│  │   (REST)      │  │    (WS)      │  │    (REST)    │        │
│  │               │  │               │  │               │        │
│  │ GET /quote    │  │ get_swap_    │  │ GET /order    │        │
│  │  ?onlyDirect  │  │   price      │  │  ?onlyDirect  │        │
│  │  Routes=true  │  │ stream_      │  │  Routes=true  │        │
│  │               │  │   quote      │  │  ?maxRoute    │        │
│  │ POST /swap-   │  │  (only_      │  │   Length=N    │        │
│  │  instructions │  │   direct_    │  │               │        │
│  │               │  │   routes)    │  │ GET /order    │        │
│  │  → Instrs     │  │  → Instrs   │  │  (w/ pubkey)  │        │
│  │  + ALTs       │  │  + ALTs     │  │  → base64 tx  │        │
│  └──────┬────────┘  └──────┬──────┘  └──────┬────────┘        │
│         │                  │                │                 │
│         ▼                  ▼                ▼                 │
│  ┌──────────────────────────────────────────────────┐        │
│  │                SwapResult                         │        │
│  │  Instructions { ixs, ALTs, CU }                  │        │
│  │  Transaction  { tx, block_height }               │        │
│  └─────────────────────┬────────────────────────────┘        │
│                        │                                      │
│                        ▼                                      │
│              into_unsigned_transaction()                      │
│                        │                                      │
│                        ▼                                      │
│              VersionedTransaction (unsigned)                   │
└──────────────────────────────────────────────────────────────┘
                         │
                         ▼
                  Caller signs & sends
```

## How the Pieces Connect

### The Two-Phase Flow: Quote → Swap

Every swap follows the same pattern:

1. **Quote phase** — Ask a provider "how much output will I get for X input?" This is read-only and doesn't require a wallet.
2. **Swap phase** — Take the quote and build executable instructions/transaction. This needs the user's public key.

The `QuoteResponse` carries an opaque `provider_data` field (JSON blob) that preserves whatever the provider needs to reconstruct the swap. For Jupiter, it's the raw quote JSON. For Dflow and Titan, it's the original request parameters (amounts, slippage, routing preferences) since the swap phase needs to issue a fresh request.

### Direct Routes vs Multi-Hop

When you swap SOL for USDC, the aggregator might route through intermediate tokens (SOL → RAY → USDC) if it finds a better price. Setting `only_direct_routes: Some(true)` on `QuoteRequest` restricts this to single-hop routes only (SOL → USDC directly).

Why would you want this? Direct routes have simpler transactions (fewer instructions, lower compute units) and less slippage risk from intermediate legs. The trade-off is potentially worse pricing since you're limiting the search space.

Each provider handles this differently under the hood:
- **Jupiter** — sends `onlyDirectRoutes` as a query parameter on the `/quote` endpoint
- **Titan** — passes it via `SwapParams.only_direct_routes` in the WebSocket request
- **Dflow** — sends it as a query parameter on `/order`, separate from `maxRouteLength` (which limits hop count without forcing single-hop)

The interesting thing: `only_direct_routes` needs to survive the quote→swap boundary. Jupiter embeds it in the raw quote JSON (which becomes `provider_data`). Titan and Dflow explicitly store it in their `provider_data` JSON blobs, then read it back during the swap phase. This is a concrete example of how `provider_data` acts as a cross-phase carrier.

### SwapResult: Two Ways to Represent a Swap

Jupiter and Titan return **instructions** + address lookup tables. You assemble these into a `VersionedTransaction` yourself. Dflow returns a **pre-built transaction** ready to sign. The `SwapResult` enum captures both:

```rust
SwapResult::Instructions { instructions, address_lookup_tables, compute_units }
SwapResult::Transaction  { transaction, last_valid_block_height }
```

The `into_unsigned_transaction()` method normalizes both into a `VersionedTransaction`.

### Feature Gates

Each provider is behind a Cargo feature flag. This means if you only use Jupiter, you don't compile the Titan WebSocket client at all:

```toml
solana-swap = { path = "../solana-swap", features = ["jupiter"] }
```

## Technologies Used

| Technology | Why |
|---|---|
| `reqwest` | HTTP client for Jupiter and Dflow REST APIs |
| `titan-rust-client` | WebSocket client for Titan's streaming API |
| `serde_json::Value` | Opaque provider data in QuoteResponse — avoids leaking internal types |
| `tokio::sync::OnceCell` | Lazy Titan WebSocket connection (only connects on first use) |
| `futures::future::join_all` | Concurrent quotes from all providers in `quote_all()` |
| `base64` + `bincode` | Deserializing Dflow's base64-encoded transactions |
| Feature flags | Compile only the providers you need |
| `cargo-nextest` | Fast test runner with per-test output control and nice reporting |
| `dotenvy` | Load `.env` files for test configuration (dev-dependency only) |

## Lessons & Gotchas

### The `provider_data` Pattern
Rather than creating separate quote types per provider and dealing with generics/trait objects, we store an opaque `serde_json::Value`. This keeps the public API simple — callers only see `QuoteResponse` — while each provider can stash whatever it needs for the swap step. The trade-off: you lose compile-time type safety on that blob. But since the same provider that writes it also reads it, this is fine in practice.

A concrete example: `only_direct_routes` is set on `QuoteRequest` during the quote phase, but Titan and Dflow also need it during the swap phase (which is a separate API call). Rather than adding provider-specific fields to `QuoteResponse`, each provider serializes `onlyDirectRoutes` into `provider_data` during `quote()` and reads it back during `swap()`. The JSON blob acts as a typed-at-runtime channel between the two phases.

### Lazy WebSocket Initialization
Titan requires a persistent WebSocket connection, but `SwapAggregator::new()` is synchronous. We use `OnceCell` to defer the async connection to the first `quote()` or `swap()` call. This means the aggregator always constructs successfully, and connection errors surface at call time.

### Address Lookup Tables
Jupiter and Titan return ALT addresses as part of their responses. We fetch the actual table data from the Solana RPC to build `AddressLookupTableAccount` structs needed for V0 message compilation. This is an extra RPC call but required for transaction size optimization.

### Dflow's Combined Endpoint
Dflow's `/order` endpoint does double duty: without `userPublicKey` it returns a quote, with it returns a quote + signed transaction. We call it twice — once for quote, once for swap — which is slightly redundant but keeps the two-phase API consistent across all providers.

Dflow also has two routing controls that interact: `onlyDirectRoutes` (boolean, direct single-hop only) and `maxRouteLength` (integer, max hops). When `maxRouteLength` is set but `onlyDirectRoutes` isn't explicitly set, we default to `onlyDirectRoutes=false` — because if you're limiting hops to 2, you clearly want multi-hop, just bounded. Explicit `only_direct_routes` from the caller always wins.

### Enum Dispatch vs Trait Objects
We chose enum dispatch (`match provider { ... }`) over `dyn SwapProvider` for simplicity. The set of providers is fixed at compile time, there's no boxing overhead, and no lifetime gymnastics with async trait methods. The `#[cfg]` attributes handle feature gating cleanly.

## Testing Strategy

### DCA Module Pattern

Rather than one flat file with all tests, we use a single binary (`tests/main.rs`) with sub-modules per provider:

```
tests/
├── main.rs                         # entry point + clippy allows
├── common/mod.rs                   # shared helpers (TestEnv, builders, finalize)
├── jupiter/
│   ├── quote_and_swap.rs           # basic flow
│   └── quote_and_swap_direct.rs    # only_direct_routes=true
├── titan/
│   ├── quote_and_swap.rs
│   └── quote_and_swap_direct.rs
└── dflow/
    ├── quote_and_swap.rs
    ├── quote_and_swap_direct.rs
    └── max_route_length.rs         # maxRouteLength=1
```

Why this pattern? In Rust, each `[[test]]` entry in Cargo.toml (or each file in `tests/`) creates a separate compilation unit. That means N test files = N compile passes of all dependencies. With one `main.rs` entry point, everything compiles into a single binary. Helper code in `common/` is shared via `crate::common::*` — no duplicate compilation.

### Nextest

We use [cargo-nextest](https://nexte.st/) as the test runner instead of `cargo test`. Config lives in `.config/nextest.toml`. Key setting: `success-output = "immediate"` — without this, nextest captures and hides stdout from passing tests, so you'd never see the formatted quote summaries and tx signatures.

All tests are `#[ignore]`'d because they hit real APIs and need env vars. Run with:

```bash
cargo nextest run --test main --run-ignored ignored-only
```

Each test prints a formatted report:
```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  jupiter::quote_and_swap  [Jupiter]
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  1000 So11..1112  →  42 EPjF..8pump
  slippage: 300 bps
  swap: 5 instructions · CU: 200000
  tx: dry run (not sent)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  jupiter::quote_and_swap: OK ✓
```

## Refer Also

- `CLAUDE.md` in this directory for build commands and implementation gotchas
- `PLAN.md` for the original design specification
- `../titan-rust-client/` for the Titan WebSocket client API
