# ADR 0001 — Contract, runtime, and migration boundaries

Status: proposed for the initial compiler and migration-analysis workstreams.

## Context

MorphRPC v1 defines a bounded frame with version, message kind, nonzero operation ID, request ID, and opaque payload. The TCP runtime exposes `Client::call(operation, payload)` and a synchronous `serve` handler that returns a response or error kind and bytes. A connection carries one in-flight request at a time. These interfaces provide the integration boundary for generated code. The frame format and existing application payloads remain governed by the v1 specification.

## Decision

The first schema compiler accepts a versioned JSON contract with this shape:

```json
{
  "schema_version": 1,
  "service": "Users",
  "messages": [
    {"name": "GetUserRequest", "fields": [{"name": "user_id", "type": "u64"}]},
    {"name": "GetUserResponse", "fields": [{"name": "name", "type": "string"}]}
  ],
  "methods": [
    {"name": "get_user", "operation_id": 1, "request": "GetUserRequest", "response": "GetUserResponse"}
  ]
}
```

Version 1 uses ordered, required fields of type `u64`, `u32`, `bool`, `string`, or `bytes`. Integers use big-endian representation. A boolean uses one byte, `0` or `1`. Strings are UTF-8 and strings and byte arrays have a four-byte unsigned big-endian byte length followed by that many bytes. A message is the concatenation of fields in contract order. Decoders reject invalid booleans, invalid UTF-8, incomplete fields, trailing bytes, and lengths exceeding the v1 frame payload limit. Encoders enforce the same limit. Compiler validation rejects unsupported versions, types and names, duplicate names and operation IDs, zero or out-of-range operation IDs, unresolved message references, and unsafe Rust identifiers. No field is implicitly optional. Contract version identifies this schema and payload convention, independently of the transport frame version.

The compiler owns the parsed contract, validated intermediate representation, and deterministic Rust generation. Generated request and response types expose payload encoding and decoding. Generated client bindings call the existing runtime and preserve its request correlation, timeout, and error behavior. Generated server bindings dispatch nonzero operation IDs into typed handlers and return an error for unknown operations or invalid payloads. No generated path silently interprets an error frame as a successful response. Async handlers, multiplexing, authentication and production transport security require later design.

The migration analyzer inspects FastAPI source without importing or executing the application. It emits a versioned, deterministic, machine-readable assessment with endpoint locations, candidate request/response mappings where statically supported, uncertainty, and blockers. Its output is an analysis artifact. Converting candidates into a compiler contract requires explicit human review, stable operation IDs, resolved types and semantic equivalence. Analysis can run without the compiler package installed. A future adapter must retain HTTP endpoints and establish compatible authentication, validation, errors, side effects, and rollback before activation.

The Performance Lab is an independent consumer of released runtime and generated-code interfaces. It owns benchmark, profiling and CI infrastructure. Comparisons of generated code require identical operations, compatible transports, reproducible inputs, and correctness checks. This ADR imposes no dependency on unfinished Performance Lab code.

## Compatibility and validation

The field encoding above is a new application payload convention for contracts that opt into it. The existing v1 envelope, operation IDs, and payloads used by existing services remain valid. In particular, the `get_user` example's response payload is not automatically compatible with a generated contract. Services must agree on the exact contract and operation IDs before communicating. Adding, removing, reordering, or changing fields can break compatibility, as can reusing an operation ID. The first compiler should reject ambiguous contracts and document compatibility limits rather than claiming automatic schema evolution.

Validate generated payloads against fixed byte fixtures, malformed and oversized inputs, and the existing frame codec. Compile generated code and exercise a client/server exchange through the existing runtime. Analyze FastAPI fixtures without network access or application execution, and verify that analysis changes no source files. Differential and shadow validation must avoid replaying side-effecting operations against production systems. Rollout and rollback remain explicit operator decisions.
