# Networking runtime

The Tokio runtime serves sequential requests on persistent TCP connections. It enforces frame limits before allocation, bounds accepted connections, and applies a deadline to connection setup and each exchange. Client calls validate correlation and close the connection after errors, timeouts, or cancellation. Server connections close on malformed or truncated input, timeout, or disconnect. Each connection supports one in-flight request. The synchronous handler must return promptly because it runs on the Tokio worker thread.

The TCP transport is unencrypted and intended for local experiments. TLS and QUIC are planned.
