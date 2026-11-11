# Architecture notes

The workspace starts with one Rust library. The protocol module owns binary encoding, framing, and compatibility rules as they are introduced. The runtime will own socket I/O, connection state, deadlines, and limits on outstanding requests. The codec should be testable directly with byte slices, independently of Tokio and the transport.

## First prototype

Use TCP on loopback to validate framing and request handling over an ordered byte stream. Specify byte order, length semantics, frame bounds, version and operation IDs, request correlation, and error responses in a language-independent wire specification before implementing the codec. Document how the runtime handles partial I/O, timeouts, malformed frames, unsupported operations, and disconnects. Plan TLS for deployments across trust boundaries and evaluate QUIC after the TCP baseline.

## Code generation and adoption

The schema compiler will emit deterministic Rust bindings from versioned service contracts. Migration tooling will preserve existing HTTP or gRPC endpoints while introducing an optional binary path for selected callers. Compare response and error behavior across both paths using fixtures or safe shadow execution. Keep AI inference in development tooling, with generated code evaluated through review, tests, and benchmarks.
