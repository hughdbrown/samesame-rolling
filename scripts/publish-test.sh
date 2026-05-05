#!/bin/sh

set -euo pipefail

cargo check --release && \
    cargo clippy --release -- -D warnings && \
    cargo fmt --check && \
    cargo test --release

cargo publish --dry-run
cargo package

