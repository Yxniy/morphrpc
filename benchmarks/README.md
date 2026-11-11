# Benchmark suite

The suite compares three implementations of the same `get_user` lookup over unencrypted loopback TCP. MorphRPC uses the v1 codec and Tokio runtime, gRPC uses tonic and Protocol Buffers over HTTP/2, and HTTP uses axum and JSON over HTTP/1.1. These measurements describe this local setup. Production comparisons require representative machines, real networks, TLS, authentication, and application workloads.

## Run

Install stable Rust and run the following from the repository root. The benchmark builds a bundled `protoc` for the host, so a separate Protocol Buffers installation is unnecessary. Use a quiet machine and avoid other workloads during a run.

```sh
cargo run --release --locked -p morphrpc-bench -- run benchmarks/configs/representative.toml /tmp/morphrpc-results
```

Results are written to `results.json` and `results.csv` in the specified directory. An existing report in that directory is overwritten. The JSON includes the configuration, UTC timestamp, OS and kernel from `uname -srvm`, CPU model when available, the compiler used to build the executable, the runtime `rustc` version, Git revision and dirty status, and the complete Cargo lockfile. Git metadata and the runtime compiler come from the working directory and `PATH` at execution time. Run through `cargo run --release --locked` from the intended clean checkout to associate those fields with the binary. CSV contains one measurement per row. Metadata commands need to be available on `PATH` and report null if unavailable. Capture the JSON with the CSV for reproducibility.

`configs/smoke.toml` is a fast functional check. `configs/default.toml` increases the request count. `configs/scheduled.toml` exercises a fixed aggregate offered rate and captures scheduling delay. Each configuration controls total measured and warm-up requests, repetitions, connection counts, and response data size. The profiles use 0, 1,024, and 65,536 bytes of response data. The size limit is 900,000 bytes. Each request has one user ID and a requested response size. A successful response contains the same ID, name, and deterministic ASCII data in every implementation. The service returns the same application error for unknown users, with protocol-specific error status encoding.

## Method

The driver starts each server in a separate process and waits for it to listen. Each worker owns one MorphRPC stream, one HTTP client, or one independent gRPC channel. Workers warm up before measurement, synchronize, and send their assigned requests sequentially. Concurrency 1 uses one worker and concurrency 8 uses eight, with at most one in-flight request per worker. The HTTP client and gRPC channel reuse healthy connections, but may reconnect; the harness does not count or verify actual connections. HTTP uses HTTP/1.1 and gRPC uses HTTP/2 without concurrent requests on each channel. The gRPC and HTTP clients and the gRPC server explicitly disable `TCP_NODELAY`. MorphRPC sockets and the axum listener use the operating system default, which was disabled on the benchmarked macOS host. Other platforms must verify that socket default before comparing results. Disabling Nagle with `TCP_NODELAY` changes packet batching and can change latency and throughput, particularly alongside delayed acknowledgments. Tonic enables this option by default, so the baseline overrides its normal behavior. [Tokio socket option](https://docs.rs/tokio/1.54.1/tokio/net/struct.TcpStream.html#method.set_nodelay), [tonic 0.12 server option](https://docs.rs/tonic/0.12.3/tonic/transport/struct.Server.html#method.tcp_nodelay). Each run uses the same loopback host, security setting, application logic, and request distribution. Protocol order rotates between repetitions. Server processes stay alive across runs, so their caches and retained memory may depend on earlier profiles.

The default load is closed-loop. Each worker sends its next request after its prior response is received and verified. This measures latency under a fixed number of outstanding requests, but it hides the queueing that would arise with independent arrivals during a stall. For a fixed-rate test, set `target_rps` to the aggregate rate. The driver assigns uniformly spaced arrivals round-robin to workers and records both service latency and latency from the scheduled arrival time. A worker sends overdue requests as soon as it can, so scheduled latency includes its accumulated backlog. The assignment is fixed to each connection rather than a shared queue, and scheduler granularity and load-generator capacity constrain accuracy. Compare requested and achieved throughput to identify overload. Failures and timeouts abort the run rather than being silently excluded.

Latency starts before client encoding and ends after the response has been read and decoded. The driver checks the returned value after the latency sample. Throughput is completed requests divided by wall time from worker release to the last verified response, including response validation and scheduling overhead. Worker connection teardown and post-run sample aggregation are excluded. Workers connect concurrently before warm-up. A 30-second end-to-end timeout covers each call, including response decoding. p50, p95, p99, and p99.9 are nearest-rank percentiles of individual samples in each repetition. Startup and per-connection warm-up are excluded. With 2,000 requests, p99 depends on roughly the slowest 20 samples and p99.9 on only the slowest two. The maximum is one observation. Three repetitions sharing server processes do not establish stable extreme-tail behavior. Use much larger sample counts, independent process starts, confidence intervals, and repeated runs when evaluating changes.

`payload_size` is the logical size of the generated data field. Request and response body sizes are the actual serialized MorphRPC payload, Protobuf message, or JSON body for the measured request. `request_framing_bytes` and `response_framing_bytes` count the 16-byte MorphRPC prefix and header or the five-byte gRPC message envelope. HTTP header/framing sizes are dynamic and left null. The gRPC envelope count excludes HTTP/2 DATA-frame headers, HPACK-compressed request and response metadata, SETTINGS, flow-control frames, and trailers, whose sizes vary with connection state. These numbers exclude HTTP/2 frames and headers, TCP/IP headers, acknowledgments, retransmissions, and link-layer bytes. `transport_bytes` is null because the suite does not capture packets. Measure actual network traffic with suitable external tooling and report its capture point.

On macOS and Linux, `server_rss_bytes` is a `ps` resident-set snapshot after the run. It is neither peak memory nor client memory and includes memory retained from earlier runs. CPU usage is null because this suite does not collect per-process CPU time over the measured interval. No zero value is substituted for an unavailable measurement. The driver and the server share one host, so client CPU, validation, scheduler contention, socket buffers, and TCP settings can limit the observed throughput. Use separate hosts, CPU profiling, repeated experiments, and packet capture to isolate bottlenecks before drawing production conclusions.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
```

The service integration test compares successful results across all three protocols for both users and all payload sizes, then checks an unknown-user error. It reuses a connection for multiple requests. The core runtime tests cover framing, partial I/O, deadlines, and disconnects.

The [November 2026 loopback baseline](results/2026-11-10-apple-m5-pro/README.md) includes measured results, limitations, and proposed follow-up investigations.

The [TCP_NODELAY diagnostic](experiments/README.md) records a controlled investigation of the original gRPC 64 KiB outliers. Its per-protocol setting changes are for diagnosis and are not comparable across protocols.

The checked-in historical baseline is about 200 KiB including a duplicated lockfile and JSON/CSV representations. This small dataset preserves provenance for the original findings. Store future large raw traces and repeated-run datasets as compressed CI artifacts or release attachments, with checksums and a concise versioned summary in Git.
