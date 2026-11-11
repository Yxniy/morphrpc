# Benchmarks

Start with equivalent operations and payloads over MorphRPC, HTTP/JSON, and gRPC/Protocol Buffers. Record payload sizes, concurrency, connection reuse, transport security, warm-up, run duration, dependency versions, hardware, and network topology. Run separate loopback and cross-host experiments so each result reflects its conditions.

Report p50, p95, and p99 latency, throughput, and transferred bytes. Capture CPU and memory when the measurement environment supports them. Publish commands and configuration alongside results so others can reproduce the comparison.
