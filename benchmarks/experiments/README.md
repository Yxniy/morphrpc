# gRPC 64 KiB TCP_NODELAY diagnostic

On November 10, 2026, an Apple M5 Pro running Darwin 27.2.0 and Rust 1.99.1 was used to repeat the 64 KiB, eight-connection gRPC workload from the original benchmark at commit `e25c131`. Each configuration used three repetitions of 2,000 requests after 300 warm-up requests. The client and server ran on the same host over unencrypted loopback. `TCP_NODELAY` was varied only for gRPC. These are diagnostic runs, not a cross-protocol comparison. The patch also logs calls exceeding 5 ms, which can affect timing when a stall occurs.

| Client TCP_NODELAY | Server TCP_NODELAY | gRPC throughput by repetition, requests/s | Maximum call latency by repetition, ms |
| --- | --- | --- | --- |
| off | off | 40,883; 5,277; 43,114 | 0.374; 235.156; 0.394 |
| off | on | 46,075; 45,822; 45,876 | 0.316; 0.355; 0.340 |
| on | off | 5,234; 5,251; 5,287 | 235.780; 235.146; 236.639 |
| on | on | 49,048; 49,189; 47,244 | 0.312; 0.316; 0.359 |

The 235 ms stalls were observed inside individual calls, including well before the last request on a connection. The server setting correlates with their disappearance in these few runs. Nagle buffering and delayed acknowledgments are a plausible interaction when `TCP_NODELAY` is off, but these results do not establish the mechanism. Capture packet and HTTP/2 traces, compare repeated runs around the 64 KiB boundary, and profile both processes before drawing conclusions about flow control, server behavior, or throughput. The original elapsed-time calculation also included worker teardown and result collection, so its throughput has additional measurement bias. The raw per-repetition values are in [grpc-nodelay-observations.csv](grpc-nodelay-observations.csv).

To repeat the diagnostic, create a disposable worktree at the recorded commit and apply [grpc-nodelay.patch](grpc-nodelay.patch). Substitute the absolute path to this directory for `EXPERIMENTS`. The patch is deliberately limited to this diagnostic and must be applied to the recorded commit. It adds environment-controlled gRPC client and server socket settings and logs slow calls.

```sh
EXPERIMENTS="$(pwd)/benchmarks/experiments"
git worktree add --detach /tmp/morphrpc-nodelay e25c131
cd /tmp/morphrpc-nodelay
git apply "$EXPERIMENTS/grpc-nodelay.patch"
cargo build --release --locked -p morphrpc-bench
for config in 00 01 10 11; do
  GRPC_CLIENT_NODELAY=${config%?} GRPC_SERVER_NODELAY=${config#?} \
    target/release/morphrpc-bench run "$EXPERIMENTS/grpc-nodelay.toml" "/tmp/morphrpc-nodelay-$config"
done
```

The two environment settings are inherited by the child server. Record the host, compiler, exact patch, configuration, and the full output when repeating this diagnostic. The patched binary reports a dirty worktree, as expected. The output directories contain measurements for all three protocols, but only the gRPC rows in this experiment vary the settings and support the within-gRPC comparison.

## Client request encoding

The [client request encoding experiment](client-encode/README.md) profiles the two request-encoding calls and compares a single-encode candidate against the unchanged runtime.
