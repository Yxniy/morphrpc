# MorphRPC

MorphRPC is an open-source project for generating, optimizing, and validating binary RPC protocols for machine-to-machine communication. The goal is to let developers describe what services exchange and generate typed clients and servers that preserve the existing service contract.

The initial workspace provides a protocol compatibility check and a test. The networking runtime, code generator, and migration tooling are planned.

## Why machine-native protocols

Services with known schemas and stable traffic patterns can sometimes encode messages in fewer bytes and spend less CPU parsing or allocating memory. The gains depend on message shape, request size, network conditions, and how services use the transport. MorphRPC will measure equivalent operations against HTTP/JSON and gRPC/Protocol Buffers with comparable connection reuse and security settings. [Benchmark methodology](benchmarks/README.md)

## AI-assisted development

Developers will supply service contracts, representative inputs, and performance constraints. Development-time tooling will propose schema changes, encoders, client and server bindings, or adapters for existing services. Generated code will be reviewable and evaluated with reproducible builds, compatibility and differential tests, and benchmarks before adoption. Deployed services will use deterministic compiled code. AI inference stays outside the network path.

An existing service can retain its HTTP or gRPC endpoints while adopters introduce a MorphRPC adapter for selected callers. Contract tests and opt-in rollout will help validate each migration and preserve a rollback path.

## Planned architecture

| Component | Role |
| --- | --- |
| [Binary protocol](src/protocol/) | Versioned frames and deterministic binary encoding with explicit compatibility rules. |
| [Networking runtime](src/runtime/) | Asynchronous Rust and Tokio client and server over TCP, with TLS and QUIC planned. |
| Schema compiler and CLI | Generate typed bindings from service contracts and run validation and benchmark workflows. |
| [Benchmarks](benchmarks/) | Reproduce measurements for latency, throughput, CPU, memory, and transferred bytes. |
| [Migration skills](skills/) | Inspect existing services, propose adapters, and validate behavioral parity during rollout. |

See the [architecture notes](docs/architecture.md) for implementation boundaries and initial protocol design questions.

## Development

Use stable Rust with `rustfmt` and `clippy` installed.

```sh
rustup toolchain install stable --profile minimal --component rustfmt --component clippy
cargo +stable fmt --all -- --check
cargo +stable clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo +stable test --workspace --all-features --locked
```

## Roadmap

1. Specify versioned, length-prefixed framing and build a local Tokio TCP `get_user` example with bounded messages, request correlation, and malformed-input tests.
2. Publish reproducible baselines against equivalent HTTP/JSON and gRPC services.
3. Generate typed clients and servers from a shared schema and test compatibility across versions.
4. Pilot AI-assisted migration for selected FastAPI endpoints with differential tests and opt-in rollout.
5. Evaluate workload-specific encodings and add secure transports, starting with TLS and then exploring QUIC.

See the [contributing guide](CONTRIBUTING.md) and [code of conduct](CODE_OF_CONDUCT.md) to participate. MorphRPC is licensed under the [Apache License 2.0](LICENSE).
