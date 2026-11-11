# Client request encoding experiment

`Client::call` encoded each request before taking the stream to reject invalid local input. Its socket writer then encoded the same frame again. The candidate writes the validated bytes directly, removing one frame buffer allocation and one copy of the request payload per valid exchange at the source level. It still uses the v1 encoder and a single `write_all` under the existing exchange deadline. No wire-format changes are involved. The runtime regression test verifies that invalid operation IDs and oversized requests preserve the connection and request ID. The golden fixtures, partial-I/O, timeout, and disconnect tests cover the exchange behavior.

## Diagnostic profile

On macOS 27.2 (26B123), Apple M5 Pro, and Xcode 27.2 (27B52), Instruments Time Profiler recorded the unchanged baseline driver at a 1 ms sample interval. Of 25,003 driver samples, 2,435 contained a `Client::call` frame. Of those, 17 included `protocol::encode`, with 12 outside `write_frame` and five inside it. Another 17 included `protocol::decode`. The sample counts show both encoder paths and suggest that eliminating one call may have a small end-to-end effect. They are inclusive stack counts and cannot establish CPU time saved or count allocations. Instruments Allocations failed to attach on this host, so the experiment has no dynamic allocation count. [CPU sample counts](cpu-samples.csv)

Use the [profiling commands](../../../docs/profiling.md) in a worktree of baseline commit `bc858a212364ddec0a3a82f1fecbd92ddbc1f95e`. Copy [the experiment config](../../configs/client-encode.toml) from this branch first because it was added after that commit. The CPU profile increases the measured request count to 200,000 per workload. The `time-profile` XPath was `/trace-toc/run[@number="1"]/data/table[@schema="time-profile"]`. Count each exported sample once by its backtrace, resolving XML references and matching the Rust symbols for `Client::call`, `protocol::encode`, `protocol::decode`, and `write_frame`.

## Release comparison

The A/B test used uninstrumented builds with Rust 1.99.1 on the same Mac. The unchanged baseline and the candidate used the same audited harness and lockfile. Each invocation measured 20,000 requests after 1,000 warmup requests at one and eight persistent connections per protocol. The workload was closed-loop TCP on the local host, with no TLS and a zero-byte response data field. There were 12 pairs, six in each order. The MorphRPC request and response each had a 12-byte body and 16-byte framing in both builds.

```sh
out=$(mktemp -d)
python3 benchmarks/lab/perf_lab.py compare \
  bc858a212364ddec0a3a82f1fecbd92ddbc1f95e 3e9aabf34878e8aca879ef4e4290ccf9823dbfa3 \
  --config benchmarks/configs/client-encode.toml --pairs 12 --output "$out"
```

| Connections | Metric | Baseline median ± SD | Candidate median ± SD | Paired change [96.1% interval] |
| ---: | --- | ---: | ---: | ---: |
| 1 | Throughput (req/s) | 62,799 ± 2,026 | 64,254 ± 3,512 | +2.76% [−1.55%, +4.45%] |
| 1 | p50 (µs) | 15.58 ± 0.11 | 15.50 ± 0.15 | −0.40% [−1.85%, +0.27%] |
| 1 | p95 (µs) | 20.21 ± 2.76 | 19.56 ± 5.91 | −3.19% [−12.16%, +2.82%] |
| 1 | p99 (µs) | 29.35 ± 3.85 | 29.02 ± 13.83 | −1.23% [−11.83%, +2.85%] |
| 8 | Throughput (req/s) | 145,476 ± 3,693 | 146,362 ± 6,023 | +0.65% [+0.16%, +1.12%] |
| 8 | p50 (µs) | 53.25 ± 1.26 | 52.96 ± 1.79 | −0.47% [−1.56%, −0.08%] |
| 8 | p95 (µs) | 85.27 ± 3.07 | 84.71 ± 6.79 | −0.85% [−1.17%, +0.79%] |
| 8 | p99 (µs) | 100.77 ± 4.76 | 99.92 ± 15.72 | −1.02% [−2.55%, +0.80%] |

The eight-connection run showed a small throughput and p50 improvement on this host. All one-connection and tail-latency intervals included zero. The gRPC and HTTP control throughput intervals also included zero, with ranges comparable to the MorphRPC change. Overall the end-to-end performance effect is inconclusive. These are per-metric intervals for one host and tiny requests. Server RSS snapshots do not measure the removed client allocation. [Raw observations](observations.csv) · [Comparison statistics](comparison.csv) · [Run manifest](manifest.json)
