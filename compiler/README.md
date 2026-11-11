# MorphRPC contract compiler

This standalone Cargo package parses a versioned service contract and emits Rust message types, payload codecs, a typed client, and synchronous server dispatch for the existing MorphRPC runtime. It generates ordinary source files at development time. Contracts must be reviewed and shared by both peers before use.

## Generate code

From the repository root, use stable Rust and a checked-in dependency lockfile.

```sh
cargo run --manifest-path compiler/Cargo.toml --locked -- compiler/examples/users.json > generated.rs
```

The generator writes diagnostics to standard error and returns a nonzero status for an invalid contract. Its output is deterministic for the same validated JSON contract and compiler version. Import the generated file as a Rust module in a crate that depends on `morphrpc` and uses Rust edition 2021 or later. Inspect [the integration fixture](tests/generated_project/src/lib.rs) for a working client, handler, and server exchange.

The generated `UsersClient::new` takes a connected `morphrpc::runtime::Client`. Each generated async method encodes its request, delegates the exchange to the runtime, and decodes a successful response. `RpcError` distinguishes payload, transport, remote error bytes, and unexpected response failures. The generated `UsersService` trait receives decoded request types synchronously. Pass its generated `dispatch` function to `morphrpc::runtime::serve` or `serve_connection` inside a closure. Dispatch returns `invalid payload`, `unknown operation`, `handler error`, or `response too large` error frames. Handler error strings are suppressed from the wire. Define application-specific error semantics before depending on them.

## Contract and payload format

The JSON structure is illustrated in [users.json](examples/users.json). Version 1 supports named messages and methods, nonzero unique 16-bit operation IDs, and ordered required fields of type `u64`, `u32`, `bool`, `string`, and `bytes`. Empty field lists are supported. The compiler checks duplicate fields, messages, methods and operation IDs, references, unknown fields, type names, and schema version. Contract input is limited to 1 MiB.

Service and message names use ASCII `[A-Z][A-Za-z0-9]*`. Fields and methods use ASCII `[a-z][a-z0-9_]*`, excluding Rust keywords. Method `new` and message names that collide with generated or standard types are reserved. Compiler validation reports conflicts before generating code.

Unsigned integers are fixed-width big-endian. A boolean is one byte with value 0 or 1. Strings are UTF-8, and strings and bytes have a four-byte big-endian length followed by that many bytes. Fields appear in declared order. Decoders reject truncated, oversized, malformed, invalid UTF-8, invalid boolean, and trailing data. The maximum encoded payload is `morphrpc::protocol::MAX_FRAME_LEN - 12` bytes, matching the v1 frame body limit after its header.

The payload convention applies to services that explicitly use these contracts. Existing v1 framing and established application payloads are unchanged. In particular, this schema's encoded string response differs from the existing `get_user` example. Field additions, removals, reordering, type changes, and operation-ID reuse can break compatibility. Schema version 1 does not provide automatic schema evolution or protocol negotiation. Review and version the service contract as a whole.

## Validation

The compiler and its generated-code fixture are nested standalone Cargo workspaces. Run their checks explicitly from the repository root.

```sh
cargo fmt --manifest-path compiler/Cargo.toml -- --check
cargo clippy --manifest-path compiler/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path compiler/Cargo.toml --locked
cargo fmt --manifest-path compiler/tests/generated_project/Cargo.toml -- --check
cargo clippy --manifest-path compiler/tests/generated_project/Cargo.toml --all-targets --locked -- -D warnings
```

The compiler test invokes a separately locked fixture build, generates Rust at build time, checks golden bytes and malformed inputs, and exercises generated client/server calls using persistent TCP. The root workspace's existing CI commands do not discover this standalone workspace. Add its explicit checks to CI when coordinating with the Performance Lab owner.
