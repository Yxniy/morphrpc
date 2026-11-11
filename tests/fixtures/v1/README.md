# MorphRPC v1 golden frames

These binary files are hand-authored from the [v1 wire specification](../../../docs/wire-format-v1.md). The integration tests use their literal bytes as the compatibility reference. Review any change to an existing fixture against the specification and document the compatibility impact in the PR.

| File | Hex bytes (prefix, header, payload) |
| --- | --- |
| `request.bin` | `00 00 00 0e  01 00 00 01  00 00 00 00 00 00 00 02  aa bb` |
| `response.bin` | `00 00 00 10  01 01 12 34  01 23 45 67 89 ab cd ef  00 7f 80 ff` |
| `error.bin` | `00 00 00 0c  01 02 ff ff  ff ff ff ff ff ff ff ff` |
| `empty-request.bin` | `00 00 00 0c  01 00 00 01  00 00 00 00 00 00 00 00` |

Use `xxd -g 1 tests/fixtures/v1/request.bin` to inspect a fixture.
