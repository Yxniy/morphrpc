# Performance Lab

Compare two commits from the same Git repository on one macOS or Linux host. Install the pinned Rust toolchain and Python 3.9 or newer. Use a quiet host, leave CPU and power settings fixed, and choose a workload before collecting data. The lab builds and executes both revisions. Use trusted revisions locally and disposable GitHub-hosted runners for PR code.

```sh
out=$(mktemp -d)
python3 benchmarks/lab/perf_lab.py compare BASELINE_SHA CANDIDATE_SHA \
  --config benchmarks/configs/representative.toml --pairs 8 --output "$out"
python3 benchmarks/lab/perf_lab.py report "$out"
```

Use `smoke.toml` with `--pairs 2` for a quick run. Use `scheduled.toml` to measure latency from scheduled arrival at a fixed offered rate. For an estimate of host noise, run the same commit on both sides using the workload and pair count planned for the real comparison. Use a fresh output directory each time. On hosts where delayed ACKs make the longer workloads slow, choose a shorter workload or increase `--run-timeout-seconds` (default 1,800 per invocation).

Both release builds use the same Rustup compiler, locked dependencies, and separate temporary targets and detached worktrees. The lab copies one configuration file and checks that the benchmark source, build script, manifest, and service schema match between revisions. Keep timed harness changes separate from runtime changes so both revisions can run the updated harness. The lab checks the reported commit, clean worktree, build compiler, configuration, host metadata, and complete workload grid before computing deltas. Both runs in a pair finish before the next pair begins. The first pair runs baseline then candidate and the next runs candidate then baseline. Use an even number of pairs to balance the order. Each invocation creates fresh server processes and warms connections using the same configuration. Builds, warmup, and profiling run outside the timed measurements.

## Outputs

| File | Contents |
| --- | --- |
| `manifest.json`, `config.toml` | Exact commits, compiler, runner class, selected build flags, workload and harness hashes, and run order. |
| `pair-NN/{baseline,candidate}/` | Raw JSON and CSV per invocation and execution logs. JSON includes the lockfile. |
| `observations.csv` | All observations with pair, execution position, revision, and side. |
| `comparison.json`, `comparison.csv` | Per-workload medians, sample standard deviations, paired median percentage changes, sample counts, and interval bounds. |
| `summary.md` | Comparison for a PR or CI job summary. |

Each invocation contributes one value per metric and workload. When the harness has several repetitions, use their median for that invocation. The change is the median of `100 × (candidate − baseline) / baseline` across pairs. A positive throughput change means more requests per second. A positive latency change means longer latency. Per-metric intervals use binomial order statistics and have at least 95% nominal coverage for the median under independent pairs. The report gives the actual nominal coverage when an interval is available. Six valid pairs are the minimum. Treat intervals as descriptive when host drift or autocorrelation undermines independence. They are per metric, with no adjustment for multiple comparisons. Check raw order and control workloads, and repeat on a quiet host when the interval spans both improvement and regression or when control workloads also shift.

The lab preserves serialized body and known framing sizes. Server RSS is a post-run snapshot. CPU usage and packet-level transport bytes remain unavailable in the harness. Unavailable measurements are null in JSON and empty in CSV. Use [profiling](profiling.md) for CPU time, allocation stacks, and socket activity.

## GitHub Actions

The separate Benchmarks workflow runs the smoke workload for PRs, comparing GitHub's merge revision with the exact PR base. Weekly and manual runs on `main` use the shorter `hosted-closed.toml` and `hosted-scheduled.toml` profiles and compare against the first parent by default. These profiles preserve payload sizes, connection counts, and socket settings while bounding the runtime on Linux. Their tail percentiles use few requests per block. Use longer workloads for a production decision. A manual run can select a baseline commit reachable from `main`. When the base has no compatible harness, the workflow reports candidate measurements without deltas. Build or runtime errors leave logs in the artifact. Hosted timings are advisory and carry no pass/fail threshold. Download `benchmark-results` to regenerate the report locally.

## Interface for an AI coding agent

Use the same CLI and artifact formats as a developer. A task should supply an approved baseline SHA, a workload file, the metric to evaluate, and any compatibility or resource constraints. Give the agent read access to raw benchmark artifacts and profiler exports. A proposal should cite the relevant functions and observations, predict one effect, and describe which existing contracts and tests apply.

On a feature branch, the agent may implement that one change, add targeted tests, run the [correctness checks](development.md#before-opening-a-pr), and call `perf_lab.py compare` on the committed candidate. It can return the patch, check results, manifest, raw measurements, and a PR description that states the result and its uncertainty. Keep profiler runs separate from the reported timings. The agent submits a PR only when asked. An independent reviewer decides whether to adopt the candidate, including when the measurements are inconclusive. The lab has no code-generation, publishing, or merge operation.
