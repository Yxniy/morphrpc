# MorphRPC engineering guidelines

- Read the [README](README.md) and [architecture notes](docs/architecture.md) before changing module boundaries or protocol behavior. Match the requested milestone and add crates only when there is a concrete need for an independent package.
- Prefer safe, idiomatic Rust and a small dependency set. Use Tokio for runtime I/O. Keep codec logic independent of sockets and bound peer-controlled allocations and in-flight requests.
- Specify byte order, frame bounds, version handling, and error behavior before making wire-format changes. Treat changes to an established format as compatibility decisions and cover them with tests.
- Keep AI analysis, generation, and optimization in development tooling. Make generated output reproducible and reviewable. Use compatibility and differential tests before introducing migration paths. Restrict shadow execution to operations with safe side effects or isolated environments.
- Work on a feature branch and submit changes through a pull request. Never merge your own PR before the required checks pass and an independent reviewer approves.
- Add targeted tests for behavior changes. Test parsing changes with malformed inputs and boundary values. Cover partial I/O, timeouts, and disconnects when changing transport behavior.
- Preserve backwards compatibility with the documented v1 wire format and its golden fixtures. Document any intentional protocol or security change and its impact in the PR and relevant specifications.
- Compare equivalent workloads and comparable transport settings when measuring performance. Record reproduction commands, environment details, and observed results. Never invent benchmark results.
- Run `cargo +stable fmt --all -- --check`, `cargo +stable clippy --workspace --all-targets --all-features --locked -- -D warnings`, and `cargo +stable test --workspace --all-features --locked` before submitting Rust changes.
- Reproduce applicable CI checks locally with the repository toolchain before submitting. Report any checks you could not run. See the [development and CI guide](docs/development.md).
