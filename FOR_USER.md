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
┌──────────────────────────────────────────────────────────┐
│                     SwapAggregator                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │
│  │  Jupiter     │  │   Titan     │  │   Dflow     │      │
│  │  (REST)      │  │   (WS)      │  │   (REST)    │      │
│  │              │  │              │  │              │      │
│  │ GET /quote   │  │ get_swap_   │  │ GET /order   │      │
│  │ POST /swap-  │  │   price     │  │  (no pubkey) │      │
│  │  instructions│  │ stream_     │  │              │      │
│  │              │  │   quote     │  │ GET /order   │      │
│  │  → Instrs    │  │  → Instrs   │  │  (w/ pubkey) │      │
│  │  + ALTs      │  │  + ALTs     │  │  → base64 tx │      │
│  └──────┬───────┘  └──────┬──────┘  └──────┬───────┘      │
│         │                 │                │              │
│         ▼                 ▼                ▼              │
│  ┌────────────────────────────────────────────────┐      │
│  │               SwapResult                        │      │
│  │  Instructions { ixs, ALTs, CU }                │      │
│  │  Transaction  { tx, block_height }             │      │
│  └─────────────────────┬──────────────────────────┘      │
│                        │                                  │
│                        ▼                                  │
│              into_unsigned_transaction()                  │
│                        │                                  │
│                        ▼                                  │
│              VersionedTransaction (unsigned)               │
└──────────────────────────────────────────────────────────┘
                         │
                         ▼
                  Caller signs & sends
```

## How the Pieces Connect

### The Two-Phase Flow: Quote → Swap

Every swap follows the same pattern:

1. **Quote phase** — Ask a provider "how much output will I get for X input?" This is read-only and doesn't require a wallet.
2. **Swap phase** — Take the quote and build executable instructions/transaction. This needs the user's public key.

The `QuoteResponse` carries an opaque `provider_data` field (JSON blob) that preserves whatever the provider needs to reconstruct the swap. For Jupiter, it's the raw quote JSON. For Dflow, it's the original request parameters. For Titan, it's also the request parameters since streaming produces a fresh route.

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

## Lessons & Gotchas

### The `provider_data` Pattern
Rather than creating separate quote types per provider and dealing with generics/trait objects, we store an opaque `serde_json::Value`. This keeps the public API simple — callers only see `QuoteResponse` — while each provider can stash whatever it needs for the swap step. The trade-off: you lose compile-time type safety on that blob. But since the same provider that writes it also reads it, this is fine in practice.

### Lazy WebSocket Initialization
Titan requires a persistent WebSocket connection, but `SwapAggregator::new()` is synchronous. We use `OnceCell` to defer the async connection to the first `quote()` or `swap()` call. This means the aggregator always constructs successfully, and connection errors surface at call time.

### Address Lookup Tables
Jupiter and Titan return ALT addresses as part of their responses. We fetch the actual table data from the Solana RPC to build `AddressLookupTableAccount` structs needed for V0 message compilation. This is an extra RPC call but required for transaction size optimization.

### Dflow's Combined Endpoint
Dflow's `/order` endpoint does double duty: without `userPublicKey` it returns a quote, with it returns a quote + signed transaction. We call it twice — once for quote, once for swap — which is slightly redundant but keeps the two-phase API consistent across all providers.

### Enum Dispatch vs Trait Objects
We chose enum dispatch (`match provider { ... }`) over `dyn SwapProvider` for simplicity. The set of providers is fixed at compile time, there's no boxing overhead, and no lifetime gymnastics with async trait methods. The `#[cfg]` attributes handle feature gating cleanly.

## Refer Also

- `CLAUDE.md` in this directory for build commands and implementation gotchas
- `PLAN.md` for the original design specification
- `../titan-rust-client/` for the Titan WebSocket client API
