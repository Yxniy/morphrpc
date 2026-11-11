# Development and CI

## Setup

Install [rustup](https://rustup.rs/) and a C/C++ build toolchain. On macOS use Xcode Command Line Tools. On Ubuntu install `build-essential`, `pkg-config`, and `libssl-dev` for the Cargo tool builds. The benchmark workspace supplies `protoc`.

From the repository root, run `rustup show` to install the stable release and components pinned in `rust-toolchain.toml`. A GitHub Codespace uses the digest-pinned Rust dev container and installs that toolchain, the coverage component, the fuzzing nightly, and pinned Cargo tools as the `vscode` user. The setup script also works on a local development machine with the prerequisites above.

```sh
bash .devcontainer/setup.sh
```

## Before opening a PR

Create a feature branch. Run the checks from the repository root with the pinned compiler.

```sh
cargo fmt --all -- --check
cargo fmt --manifest-path fuzz/Cargo.toml -- --check
cargo fmt --manifest-path compiler/Cargo.toml -- --check
cargo fmt --manifest-path compiler/tests/generated_project/Cargo.toml -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --all-targets --all-features --locked
cargo test --workspace --all-features --locked
cargo clippy --manifest-path compiler/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path compiler/Cargo.toml --locked
cargo build --manifest-path compiler/tests/generated_project/Cargo.toml --all-targets --locked
cargo clippy --manifest-path compiler/tests/generated_project/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path compiler/tests/generated_project/Cargo.toml --locked
python3 -m unittest discover -s skills/fastapi-assessment/tests -v
cargo build --release --workspace --all-targets --all-features --locked
```

To reproduce a protocol or networking failure, run the affected integration test. Add a test name after the relevant `--test` target to narrow the run. See the [wire specification](wire-format-v1.md) and [golden fixture bytes](../tests/fixtures/v1/).

```sh
cargo test --locked -p morphrpc --test codec --test protocol_version --test runtime
```

For workflow changes, run `actionlint` (CI uses 1.7.12) with `shellcheck` installed, then run `shellcheck .devcontainer/setup.sh`. For dependency changes, audit all four lockfiles from a directory outside the checkout so project Cargo configuration is outside the audit process.

```sh
repo=$(pwd)
(
    cd /tmp
    cargo-audit audit --file "$repo/Cargo.lock"
    cargo-audit audit --file "$repo/fuzz/Cargo.lock"
    cargo-audit audit --file "$repo/compiler/Cargo.lock"
    cargo-audit audit --file "$repo/compiler/tests/generated_project/Cargo.lock"
)
```

The audit uses the current [RustSec advisory database](https://github.com/RustSec/advisory-db). Advisory and registry data can change between runs. Check the advisory and the dependency path from the job log. Keep the lockfile repair focused and rerun the audit.

## GitHub Actions

| Workflow and check | Trigger | Merge policy and output |
| --- | --- | --- |
| CI / `Rust` | PR, merge queue, push to `main` | Require. Formatting, Clippy with warnings denied, debug and release builds, unit and integration tests, including v1 compatibility, the standalone compiler, and generated bindings. |
| CI / `Dependency audit` | PR, merge queue, push to `main`, weekly | Require. RustSec audit of workspace, fuzz, compiler and generated-project lockfiles. |
| CI / `Workflow lint` | PR, merge queue, push to `main` | Require. Validates workflow syntax, expressions, and shell scripts. |
| CI / `Migration assessment` | PR, merge queue, push to `main` | Require. Runs static analyzer unit tests and the reviewed contract handoff test with pinned Python. |
| Coverage / `Test coverage` | PR, push to `main` | Advisory. Workspace coverage totals and a `workspace-lcov` artifact. |
| Benchmarks / `Benchmark reports` | PR, weekly, manual on `main` | Advisory. PR smoke run and same-run baseline comparison when available. Weekly and manual runs use shorter hosted closed-loop and fixed-rate profiles. |
| CodeQL / `CodeQL PR report` | PR | Advisory. Rust findings in a `codeql-pr-sarif` artifact. |
| CodeQL / `CodeQL Rust` | Push to `main`, weekly | Advisory. Rust findings in the repository Security tab. |
| Fuzz / `Fuzz frame decoder` | Weekly, manual on `main` | Advisory. AddressSanitizer run with a five-minute fuzzing budget, a per-input timeout, and logs and reproducer artifacts. |
| Release validation / `Validate release artifacts` | Manual on `main` | Advisory. Verifies the crate package and stages it alongside a versioned Linux `get_user` example archive, SHA-256 checksums, and build metadata. |

In the PR **Checks** tab, open the failing job and expand the failing step. To reproduce it, check out the commit listed in the checkout step, use its toolchain and lockfiles, and run the printed command. The PR workflows test GitHub's prospective merge revision. Download artifacts from the run summary within their 14-day retention period.

Reproduce coverage locally with the same instrumented test command.

```sh
rustup component add llvm-tools-preview
cargo llvm-cov --workspace --all-features --locked --lcov --output-path lcov.info
cargo llvm-cov report --workspace --summary-only
```

For a fuzz failure, download the failing input and use the same nightly and target from the log. Seed a new local session with the golden fixtures.

```sh
corpus=$(mktemp -d)
cp tests/fixtures/v1/*.bin "$corpus/"
cargo +nightly-2026-11-01 fetch --locked --manifest-path fuzz/Cargo.toml
CARGO_NET_OFFLINE=true cargo +nightly-2026-11-01 fuzz run frame_decode "$corpus" -- \
  -max_total_time=300 -timeout=10 -rss_limit_mb=2048 -max_len=1048580 -print_final_stats=1
```

Replace `"$corpus"` with the path to a failing input to replay it. GitHub adds an outer timeout for the build and fuzz process. AddressSanitizer support depends on the platform. The workflow uses Ubuntu x86-64.

Release validation uses `cargo package --locked -p morphrpc` and `cargo build --release --locked -p morphrpc --example get_user`. The workflow only uploads Actions artifacts. Publishing, signing, and production deployment are outside the workflow. Future publication should use an explicitly authorized version and an approval-protected job with separate publishing credentials.

## Benchmarks

Run the smoke profile locally, then use the representative and scheduled profiles for longer workloads. Each run writes `results.json` and `results.csv`.

```sh
out=$(mktemp -d)
for profile in smoke representative scheduled; do
  cargo run --release --locked -p morphrpc-bench -- \
    run "benchmarks/configs/$profile.toml" "$out/$profile"
done
```

See the [Performance Lab](performance-lab.md) for paired baseline/candidate runs, report regeneration, and advisory hosted measurements. To reproduce the tooling checks, run `python3 -m unittest discover -s benchmarks/lab`. See the [profiling guide](profiling.md) for CPU, heap, and socket diagnostics.

## Configure protection for `main`

A repository administrator can create an active branch ruleset in **Settings → Rules → Rulesets**, targeting only `main`.

1. Require a pull request and one approval from a reviewer with write access. Dismiss stale approvals on new commits and, where available, require approval of the most recent reviewable push from someone other than the last pusher. A solo maintainer needs another eligible reviewer to enforce independent approval.
2. Require `Rust`, `Dependency audit`, `Workflow lint`, and `Migration assessment` from GitHub Actions. Select the exact names GitHub records after each job has run successfully. Enable **Require branches to be up to date before merging**.
3. Block force pushes and restrict deletions.
4. Leave the bypass list empty. If a recovery policy requires an exception, limit it to a designated maintainer or team and use PR-only bypass where available.

Branch rulesets are available for this public repository on GitHub Free. A merge queue is an alternative if the project moves to an eligible organization-owned repository. GitHub offers it for public organization repositories and qualifying private organization repositories on Enterprise Cloud. The CI jobs already listen for `merge_group`. Private repository ruleset availability depends on the plan. [Ruleset options](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets) · [Merge queue eligibility](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue)

## Workflow security

PR jobs run on disposable GitHub-hosted runners using `pull_request`, a read-only repository token, and checkout without persisted credentials. Keep fork workflow write tokens and secret forwarding disabled. Keep secrets and credentials out of build caches and artifacts. The CI cache contains Cargo registry downloads and index data only. Artifact uploads use GitHub's scoped artifact service. Review changes to workflow files, build scripts, lockfiles, and golden fixtures before approval. Fork submissions may require a maintainer to approve their first workflow run.

Rust CodeQL extraction can run build scripts even in `none` build mode. The PR analysis has a read-only token and uploads SARIF as an Actions artifact. Only the separate job on trusted `main` revisions has permission to upload findings to the Security tab. If those uploads fail, check repository eligibility and whether default setup conflicts with the workflow's advanced configuration. [CodeQL build options](https://docs.github.com/en/code-security/reference/code-scanning/codeql/build-options-for-compiled-languages#building-rust)

Fuzzing and release validation run on `main` with read-only repository permissions. Keep benchmark jobs for untrusted PRs on GitHub-hosted runners if dedicated infrastructure is added for trusted measurements.
