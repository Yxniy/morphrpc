# MorphRPC wire format v1

A connection carries a sequence of frames. All integers are unsigned and encoded in big-endian byte order. The length prefix counts the bytes after the prefix, including the 12-byte header and payload. A frame body has a minimum length of 12 bytes and a maximum length of 1,048,576 bytes. The four-byte prefix is excluded from that limit.

| Offset | Size | Field | Meaning |
| --- | --- | --- | --- |
| 0 | 4 | Length | Body length in bytes |
| 4 | 1 | Version | `1` |
| 5 | 1 | Kind | `0` request, `1` response, `2` error |
| 6 | 2 | Operation | Nonzero operation identifier |
| 8 | 8 | Request ID | Correlates responses and errors with requests |
| 16 | Variable | Payload | Operation-specific bytes, possibly empty |

A response or error uses the same operation and request ID as its request. The application defines operation identifiers, payload encodings, and error payloads. Unknown nonzero operations are valid frames and must be handled by the dispatcher. The codec does not interpret payload bytes. A request ID may be zero; callers are responsible for distinguishing outstanding requests on a connection.

For example, a request with operation `1`, request ID `2`, and payload `aa bb` is encoded as:

```text
00 00 00 0e  01 00 00 01  00 00 00 00 00 00 00 02  aa bb
```

A receiver checks the length as soon as the prefix is available and rejects lengths outside the permitted range before allocating a body buffer. It then reads the complete body, validates the version, kind, and operation, and passes the payload to the operation handler. Unknown versions, invalid kinds, and operation zero are invalid frames. A stream ending before the full prefix or declared body is truncated. A decoder consumes one frame at a time and leaves subsequent bytes for the next frame. A transport should close the connection on malformed, oversized, or truncated frames, and enforce read deadlines and limits on concurrent requests. Payload validation and unknown-operation errors belong to the dispatcher.

The [get_user example](../examples/README.md) defines a local operation and payload encoding using this framing. The TCP runtime uses a deadline for each complete exchange, permits one in-flight request per connection, closes failed connections, and limits concurrent connections. Clean EOF between frames ends the connection. EOF inside a frame is reported as a truncated I/O error.

The [golden fixtures](../tests/fixtures/v1/) record representative v1 bytes for requests, responses, and errors. Run `cargo test --locked -p morphrpc --test codec --test protocol_version --test runtime` to verify the encoding, version handling, frame bounds, and transport rejection behavior. For an intentional wire change, update the specification with its compatibility impact and retain fixtures for versions that remain supported.
