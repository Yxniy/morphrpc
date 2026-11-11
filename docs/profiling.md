# Profiling MorphRPC

Use a release build with debug symbols on a quiet machine. Treat profiled runs as diagnostics. Use a fresh uninstrumented build and the [Performance Lab](performance-lab.md) for timing comparisons. Keep the same workload, build settings, OS, and profiler version when comparing profiles. macOS uses kqueue and the system malloc implementation. Linux uses epoll and its host allocator. Sampling, allocator sizes, syscall names, and packet capture points differ across platforms.

Build the profiling binary from the repository root with the pinned Rust toolchain on `PATH`.

```sh
out=$(mktemp -d)
CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release --locked -p morphrpc-bench
binary="$PWD/target/release/morphrpc-bench"
```

The launched driver is the client process. It spawns one server subprocess per protocol using the same binary, so distinguish the driver from children by PID and arguments. To inspect the server, attach to a server PID during a longer run. Profilers may retain local paths or traffic, so inspect exports before sharing them.

## macOS

Install full Xcode and select its developer directory. Run `xcrun xctrace list templates` to check for Time Profiler, Allocations, and System Trace. Use local Xcode developer permissions for the target. Allocation tracing may require the target's debug entitlement even when CPU sampling works. The CPU and socket commands use tiny responses and one or eight persistent connections.

```sh
sed 's/^requests = 20000$/requests = 200000/' benchmarks/configs/client-encode.toml \
  > "$out/profile.toml"
xcrun xctrace record --template 'Time Profiler' --time-limit 90s \
  --output "$out/cpu.trace" --launch -- "$binary" run "$out/profile.toml" "$out/cpu-run"
xcrun xctrace record --template 'Allocations' --time-limit 90s \
  --output "$out/heap.trace" --launch -- "$binary" run benchmarks/configs/client-encode.toml "$out/heap-run"
xcrun xctrace record --template 'System Trace' --time-limit 90s \
  --output "$out/io.trace" --launch -- "$binary" run "$out/profile.toml" "$out/io-run"
xcrun xctrace export --input "$out/cpu.trace" --toc > "$out/cpu-toc.xml"
```

If Allocations cannot attach, sign a temporary copy of the local debug-symbol build and launch that copy.

```sh
cp "$binary" "$out/morphrpc-bench-debug"
cat > "$out/entitlements.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>com.apple.security.get-task-allow</key>
    <true/>
</dict>
</plist>
PLIST
codesign --force --sign - --entitlements "$out/entitlements.plist" "$out/morphrpc-bench-debug"
xcrun xctrace record --template 'Allocations' --time-limit 90s \
  --output "$out/heap-debug.trace" --launch -- "$out/morphrpc-bench-debug" \
  run benchmarks/configs/client-encode.toml "$out/heap-debug-run"
```

Use the signed copy for local diagnostics only. [Apple debugging entitlement guidance](https://developer.apple.com/documentation/security/resolving-common-notarization-issues)

Open the trace in Instruments and select the launched PID. Use the Time Profiler call tree to inspect inclusive and self samples. Use Allocations to inspect counts, bytes, and lifetimes grouped by stack. Include freed allocations when looking for temporary request buffers. Use System Trace to inspect scheduler waits and system calls on the driver threads. For analysis in a script, use the exported table of contents to find the CPU sample table and pass its XPath to `xctrace export`. When you need actual loopback packet sizes or retransmissions, capture the relevant server port with `tcpdump -i lo0` on macOS or `tcpdump -i lo` on Linux, subject to local capture permissions. Capture traffic separately from timing runs. [Apple CPU profiles](https://developer.apple.com/documentation/xcode/analyzing-cpu-profiles-with-call-tree-views) · [Apple Allocations](https://developer.apple.com/documentation/xcode/gathering-information-about-memory-use)

## Linux

Install the `perf` tools matching the kernel, `heaptrack`, and `strace` through your distribution. Use a host where the profiling user has access to the required perf events. Check `cat /proc/sys/kernel/perf_event_paranoid` if perf reports an access error. GitHub-hosted Ubuntu runners can restrict these events, and a runner administrator controls that setting. The following shell wrapper records the driver PID before executing the benchmark, preserving the PID for filtering. Perf inherits child processes and threads by default. Its `task-clock` totals therefore cover the driver and its children. The Linux diagnostic profile uses tiny responses and 32 connections to keep the workload practical when gRPC encounters delayed ACKs with the audited TCP settings. For a short syscall trace at lower concurrency, use the smoke profile.

```sh
cat > "$out/linux-profile.toml" <<'TOML'
requests = 4000
warmup_requests = 128
repetitions = 1
concurrency = [32]

[[profiles]]
name = "tiny"
payload_size = 0
TOML

perf stat -e task-clock,context-switches,cpu-migrations,page-faults \
  -o "$out/perf-stat.txt" -- "$binary" run "$out/linux-profile.toml" "$out/stat-run"
perf record -e cpu-clock -g --call-graph dwarf -o "$out/perf.data" -- \
  sh -c 'echo "$$" > "$1"; shift; exec "$@"' sh "$out/driver.pid" \
  "$binary" run "$out/linux-profile.toml" "$out/perf-run"
perf report --stdio --pid "$(cat "$out/driver.pid")" -i "$out/perf.data"

heaptrack -o "$out/heap" "$binary" run "$out/linux-profile.toml" "$out/heap-run"
# Pass the capture filename printed by heaptrack, including its .zst or .gz suffix.
heaptrack_print CAPTURE_FILE > "$out/heap.txt"

strace -ff -tt -T -y -e trace=%network,read,write -o "$out/socket" \
  "$binary" run benchmarks/configs/smoke.toml "$out/io-run"
```

Review heap allocation counts and temporary allocations by stack. Where the capture includes child processes, filter the driver PID. In the syscall logs, separate sockets from file I/O using the descriptor annotations and follow the relevant PIDs and threads. Syscall duration includes kernel wait time. [Linux perf permissions](https://docs.kernel.org/admin-guide/perf-security.html) · [perf record options](https://github.com/torvalds/linux/blob/master/tools/perf/Documentation/perf-record.txt) · [Heaptrack](https://github.com/KDE/heaptrack)

## Read the profiles

- Start at `morphrpc_bench::Connection::call` and the Tokio worker tasks. Separate driver work such as request creation, response validation, timers, and scheduling from socket readiness and server work.
- Inspect `morphrpc::protocol::encode`, `decode`, `morph_request`, and `morph_response`. The protocol codec handles framing, while the latter functions handle application payloads. Include inlined frames when available.
- Compare time and heap call stacks together. Allocation volume, RSS, and CPU samples describe different effects. Sample counts estimate where CPU work occurs. They do not count calls or allocations.
- Review socket reads and writes, scheduler wakeups, and the time between a request and response. Compare one connection with several to see when the driver, server, or runtime scheduler becomes limiting.
