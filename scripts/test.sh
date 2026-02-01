#!/bin/bash
# cargo test -- --no-capture --ignored
cargo nextest run --test main --run-ignored ignored-only