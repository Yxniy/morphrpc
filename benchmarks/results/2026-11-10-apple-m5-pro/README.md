# Loopback baseline, November 10, 2026

These results were recorded on an Apple M5 Pro running Darwin 27.2.0 with Rust 1.99.1, using release builds at commit `4bc5987e8dcb744dcdfd74f565b9831f897a544d`. The working tree was clean. Each closed-loop row represents the median of three repetitions of 2,000 measured requests after 300 warm-up requests. Servers and load generator ran as separate processes on the same host over unsecured loopback. The [JSON and CSV reports](closed/) contain the individual runs and environment metadata.

| Response data | Connections | MorphRPC req/s | gRPC req/s | HTTP/JSON req/s | MorphRPC p99 µs | gRPC p99 µs | HTTP p99 µs |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 B | 1 | 56,694 | 20,181 | 36,089 | 30 | 65 | 42 |
| 0 B | 8 | 167,936 | 76,310 | 113,257 | 87 | 165 | 130 |
| 1 KiB | 1 | 56,314 | 16,141 | 33,649 | 31 | 80 | 45 |
| 1 KiB | 8 | 161,572 | 66,017 | 109,029 | 92 | 180 | 134 |
| 64 KiB | 1 | 25,442 | 11,676 | 15,318 | 52 | 101 | 79 |
| 64 KiB | 8 | 92,455 | 5,206 | 63,048 | 144 | 280 | 204 |

The large response reduced MorphRPC throughput by about 45% at eight connections compared with the tiny response, while increasing median p99 from 87 to 144 µs. Response allocation, copying, serialization, transport, and client verification are all included in the workload, so this observation does not isolate a CPU or network cause. Profile allocation and copies on separate client and server hosts before changing the codec. The current client encodes a request once for validation and again for writing, which is a concrete extra allocation to evaluate in a separate controlled experiment.

At eight connections and 64 KiB, gRPC showed a median maximum latency of about 235 ms even though its median p99 was 280 µs. Its measured throughput fell to 5,206 requests/s. The recorded samples demonstrate an extreme tail in this setup, and a trace is needed to establish its cause. Investigate HTTP/2 flow control, TCP behavior, and scheduling before attributing the difference to any one component.

MorphRPC's tiny request used 12 payload bytes plus 16 frame bytes, compared with 2 Protobuf bytes plus a 5-byte gRPC message envelope. Its tiny response used 12 payload bytes plus 16 frame bytes, compared with 7 Protobuf bytes plus a 5-byte envelope. HTTP headers, HTTP/2 frames, TCP/IP, and link-layer bytes are excluded from these figures. This is a case where MorphRPC's application encoding and frame are larger for the measured operation.

The [scheduled-rate reports](scheduled/) used 5,000 aggregate requests/s for the same payloads and connection counts. In the first tiny, one-connection repetition, MorphRPC's p99 from scheduled arrival was 2,154 µs versus 1,994 µs for HTTP/JSON. Scheduler granularity and the shared load-generator host affect these results. Closed-loop throughput was higher for MorphRPC in every repetition in this particular run. These observations are local baselines and should not be generalized to secure or deployed networks.

Run more repetitions on an otherwise idle host, capture CPU profiles and allocation counts for both client and server, and repeat across hosts with packet-level measurements. Compare fixed-rate scheduling delay and saturation before and after any optimization. The benchmark methodology and exact reproduction commands are in the [suite README](../../README.md).
