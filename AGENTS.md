# MorphRPC engineering guidelines

- Read the [README](README.md) and [architecture notes](docs/architecture.md) before changing module boundaries or protocol behavior. Match the requested milestone and add crates only when there is a concrete need for an independent package.
- Prefer safe, idiomatic Rust and a small dependency set. Use Tokio for runtime I/O. Keep codec logic independent of sockets and bound peer-controlled allocations and in-flight requests.
- Specify byte order, frame bounds, version handling, and error behavior before making wire-format changes. Treat changes to an established format as compatibility decisions and cover them with tests.
- Keep AI analysis, generation, and optimization in development tooling. Make generated output reproducible and reviewable. Use compatibility and differential tests before introducing migration paths. Restrict shadow execution to operations with safe side effects or isolated environments.
- Test parsing changes with malformed inputs and boundary values. Cover partial I/O, timeouts, and disconnects when changing transport behavior. Prefer targeted tests for behavior changes.
- Compare equivalent workloads and comparable transport settings when measuring performance. Record reproduction commands, environment details, and observed results.
- Run `cargo +stable fmt --all -- --check`, `cargo +stable clippy --workspace --all-targets --all-features --locked -- -D warnings`, and `cargo +stable test --workspace --all-features --locked` before submitting Rust changes.
