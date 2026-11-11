# Client request encoding experiment

`Client::call` encoded each request before taking the stream to reject invalid local input. Its socket writer then encoded the same frame again. The candidate writes the validated bytes directly, removing one frame buffer allocation and one copy of the request payload per valid exchange at the source level. It uses the v1 encoder and a single `write_all` under the existing exchange deadline, then releases the encoded buffer before awaiting the response. The runtime regression test verifies that invalid operation IDs and oversized requests preserve the connection and request ID. The golden fixtures, partial-I/O, timeout, and disconnect tests cover the exchange behavior.

## Diagnostic profile

On macOS 27.2 (26B123), Apple M5 Pro, and Xcode 27.2 (27B52), Instruments Time Profiler recorded the unchanged baseline driver at a 1 ms sample interval. Of 25,003 driver samples, 2,435 contained a `Client::call` frame. Of those, 17 included `protocol::encode`, with 12 outside `write_frame` and five inside it. Another 17 included `protocol::decode`. The sample counts show both encoder paths and suggest that eliminating one call may have a small end-to-end effect. They are inclusive stack counts and cannot establish CPU time saved or count allocations. Both revisions completed a separate Instruments Allocations capture after signing temporary diagnostic copies. Those captures used one connection, 1,000 measured requests, and 100 warmup requests per protocol. The default allocation-list XML export contained live allocations, so it could not quantify the temporary encoding buffers. [CPU sample counts](cpu-samples.csv)

Use the [profiling commands](../../../docs/profiling.md) in a worktree of baseline commit `bc858a212364ddec0a3a82f1fecbd92ddbc1f95e`. Copy [the experiment config](../../configs/client-encode.toml) from this branch first because it was added after that commit. The CPU profile increases the measured request count to 200,000 per workload. The `time-profile` XPath was `/trace-toc/run[@number="1"]/data/table[@schema="time-profile"]`. Count each exported sample once by its backtrace, resolving XML references and matching the Rust symbols for `Client::call`, `protocol::encode`, `protocol::decode`, and `write_frame`.

## Release comparison

The November 10, 2026 A/B test used uninstrumented builds with Rust 1.99.1 on the same Mac. The unchanged baseline and the candidate used the same audited harness and lockfile. Each invocation measured 20,000 requests after 1,000 warmup requests at one and eight persistent connections per protocol. The workload was closed-loop TCP on the local host, with no TLS and a zero-byte response data field. There were 12 pairs, six in each order. The MorphRPC request and response each had a 12-byte body and 16-byte framing in both builds.

```sh
out=$(mktemp -d)
python3 benchmarks/lab/perf_lab.py compare \
  bc858a212364ddec0a3a82f1fecbd92ddbc1f95e fec60b35ec1aa85d9c33010b68947c410388de72 \
  --config benchmarks/configs/client-encode.toml --pairs 12 --output "$out"
```

| Connections | Metric | Baseline median ± SD | Candidate median ± SD | Paired change [96.1% interval] |
| ---: | --- | ---: | ---: | ---: |
| 1 | Throughput (req/s) | 61,863 ± 1,515 | 62,307 ± 1,311 | −0.91% [−2.36%, +1.74%] |
| 1 | p50 (µs) | 15.46 ± 0.18 | 15.52 ± 0.12 | +0.81% [+0.53%, +1.09%] |
| 1 | p95 (µs) | 20.46 ± 0.88 | 20.00 ± 1.40 | +1.44% [−4.59%, +9.25%] |
| 1 | p99 (µs) | 29.04 ± 2.19 | 28.62 ± 2.70 | +1.28% [−4.10%, +4.00%] |
| 8 | Throughput (req/s) | 166,717 ± 2,328 | 167,149 ± 2,095 | +0.11% [−1.65%, +0.67%] |
| 8 | p50 (µs) | 46.29 ± 0.35 | 46.23 ± 0.45 | −0.22% [−0.98%, +1.08%] |
| 8 | p95 (µs) | 74.90 ± 1.87 | 74.83 ± 1.59 | −0.33% [−1.06%, +2.28%] |
| 8 | p99 (µs) | 88.52 ± 7.45 | 88.73 ± 3.63 | −0.92% [−2.12%, +3.10%] |

The throughput and tail-latency intervals included zero at both connection counts. The paired one-connection p50 change was +0.81%, and the eight-connection p50 interval included zero. The gRPC and HTTP control throughput intervals also included zero. The experiment found a small one-connection median-latency regression on this host. The broader end-to-end performance effect is inconclusive. These are per-metric intervals for one host and tiny requests. Server RSS snapshots do not measure the removed client allocation. [Raw observations](observations.csv) · [Comparison statistics](comparison.csv) · [Run manifest](manifest.json)
