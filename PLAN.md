# Goal

- Create a unified swap crate
    - Support Jupiter ✅
    - Support Titan ✅
    - Support Dflow ✅

# Status

All providers implemented. Builds and passes clippy with all feature combinations.

# Comments

- API keys for testing via .env
- Keypair for testing via .env

# References

- Plan I already made /Users/ohaddahan/RustroverProjects/zklsol/worker-service/refactor.md
- We should build on top of /Users/ohaddahan/RustroverProjects/zklsol/titan-rust-client
- Review /Users/ohaddahan/RustroverProjects/zklsol/jupiter-cpi-swap-example
- Review https://pond.dflow.net/build/trading-api/imperative/swap#create-swap with chrome tool

# Unresolved Questions

1. **Titan dependency**: currently path `../titan-rust-client`. Needs git URL or crates.io publish for production.
2. **Dflow API key**: optional during development per docs. Need key for production rate limits.
3. **Jupiter API key**: optional for basic use but rate-limited without one.
4. **Titan quote accuracy**: `get_swap_price` returns price but no route. The streaming quote in `swap()` gets the real route. Quote amounts may differ from final swap amounts.
5. **Integration testing**: needs .env with API keys + RPC endpoint to test against live APIs.
