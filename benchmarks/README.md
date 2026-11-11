# Benchmark suite

The suite compares three implementations of the same `get_user` lookup over unencrypted loopback TCP. MorphRPC uses the v1 codec and Tokio runtime, gRPC uses tonic and Protocol Buffers over HTTP/2, and HTTP uses axum and JSON over HTTP/1.1. These measurements describe this local setup. Production comparisons require representative machines, real networks, TLS, authentication, and application workloads.

## Run

Install stable Rust and run the following from the repository root. The benchmark builds a bundled `protoc` for the host, so a separate Protocol Buffers installation is unnecessary. Use a quiet machine and avoid other workloads during a run.

```sh
cargo run --release --locked -p morphrpc-bench -- run benchmarks/configs/representative.toml /tmp/morphrpc-results
```

Results are written to `results.json` and `results.csv` in the specified directory. An existing report in that directory is overwritten. The JSON includes the configuration, UTC timestamp, `uname`, CPU model when available, Rust version, Git revision and dirty status, and the complete Cargo lockfile. CSV contains one measurement per row. Metadata commands need to be available on `PATH` and report null if unavailable. Capture the JSON with the CSV for reproducibility.

`configs/smoke.toml` is a fast functional check. `configs/default.toml` increases the request count. `configs/scheduled.toml` exercises a fixed aggregate offered rate and captures scheduling delay. Each configuration controls total measured and warm-up requests, repetitions, connection counts, and response data size. The profiles use 0, 1,024, and 65,536 bytes of response data. The size limit is 900,000 bytes. Each request has one user ID and a requested response size. A successful response contains the same ID, name, and deterministic ASCII data in every implementation. The service returns the same application error for unknown users, with protocol-specific error status encoding.

## Method

The driver starts each server in a separate process, waits for it to listen, and creates one persistent client connection per worker. Each worker warms that same connection before measurement. Workers then synchronize and send their assigned requests sequentially. Thus concurrency 1 measures sequential requests on one connection, and concurrency 8 uses eight connections with at most one in-flight request each. gRPC has one independent channel per worker, with no extra multiplexed requests per channel. HTTP uses persistent HTTP/1.1. All clients and servers disable `TCP_NODELAY`, matching the current MorphRPC runtime. Each run uses the same loopback host, security setting, application logic, and request distribution. Protocol order rotates between repetitions. Server processes stay alive across runs, so their caches and retained memory may depend on earlier profiles.

The default load is closed-loop. Each worker sends its next request after its prior response is received and verified. This measures latency under a fixed number of outstanding requests, but it hides the queueing that would arise with independent arrivals during a stall. For a fixed-rate test, set `target_rps` to the aggregate rate. The driver assigns uniformly spaced arrivals round-robin to workers and records both service latency and latency from the scheduled arrival time. A worker sends overdue requests as soon as it can, so scheduled latency includes its accumulated backlog. The assignment is fixed to each connection rather than a shared queue, and scheduler granularity and load-generator capacity constrain accuracy. Compare requested and achieved throughput to identify overload. Failures and timeouts abort the run rather than being silently excluded.

Latency starts before client encoding and ends after the response has been read and decoded. The driver checks the returned value after the latency sample. Throughput is completed requests divided by wall time from worker release through completion, including response validation and scheduling overhead. p50, p95, p99, and p99.9 are nearest-rank percentiles of individual samples in each repetition. Startup and per-connection warm-up are excluded. A small number of requests gives unstable tail estimates, so use the larger configuration and multiple independent runs when evaluating changes.

`payload_size` is the logical size of the generated data field. Request and response body sizes are the actual serialized MorphRPC payload, Protobuf message, or JSON body for the measured request. `request_framing_bytes` and `response_framing_bytes` count the 16-byte MorphRPC prefix and header or the five-byte gRPC message envelope. HTTP header/framing sizes are dynamic and left null. These numbers exclude HTTP/2 frames and headers, TCP/IP headers, acknowledgments, retransmissions, and link-layer bytes. `transport_bytes` is null because the suite does not capture packets. Measure actual network traffic with suitable external tooling and report its capture point.

On macOS and Linux, `server_rss_bytes` is a `ps` resident-set snapshot after the run. It is neither peak memory nor client memory and includes memory retained from earlier runs. CPU usage is null because this suite does not collect per-process CPU time over the measured interval. No zero value is substituted for an unavailable measurement. The driver and the server share one host, so client CPU, validation, scheduler contention, socket buffers, and TCP settings can limit the observed throughput. Use separate hosts, CPU profiling, repeated experiments, and packet capture to isolate bottlenecks before drawing production conclusions.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
```

The service integration test compares successful results across all three protocols for both users and all payload sizes, then checks an unknown-user error. It reuses a connection for multiple requests. The core runtime tests cover framing, partial I/O, deadlines, and disconnects.
