#!/usr/bin/env bash
set -euo pipefail

rustup show
rustup component add llvm-tools-preview
rustup toolchain install nightly-2026-11-01 --profile minimal
(
    # Cargo tool installation uses each tool's upstream lockfile.
    cd /tmp
    cargo +1.99.1 install --locked cargo-audit --version 0.22.2
    cargo +1.99.1 install --locked cargo-llvm-cov --version 0.9.1
    cargo +1.99.1 install --locked cargo-fuzz --version 0.13.2
)
cargo fetch --locked
cargo +nightly-2026-11-01 fetch --locked --manifest-path fuzz/Cargo.toml
