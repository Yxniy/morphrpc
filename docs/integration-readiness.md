# Integration readiness

Review snapshot for the MorphRPC implementation and CI pull requests on November 11, 2026. The integration branch combines the work for validation. Individual PR heads, `main`, CI results, and GitHub settings can change; recheck them at each merge.

## PR relationships

- [Architecture PR #3](https://github.com/Yxniy/morphrpc/pull/3) defines the contract and runtime boundaries.
- [Performance Lab PR #4](https://github.com/Yxniy/morphrpc/pull/4) contains the head commit of [CI/security PR #2](https://github.com/Yxniy/morphrpc/pull/2). PR #4 retains the v1 golden fixtures, correctness, audit, workflow, coverage, fuzz, CodeQL, and release-validation work from #2. Its Performance Lab replaces #2's benchmark-summary script with the paired report implementation. Review that replacement and the final PR #4 head; do not merge #2 separately.
- [Compiler PR #5](https://github.com/Yxniy/morphrpc/pull/5) adds a standalone Cargo workspace. The root workspace alone does not test it.
- [Migration PR #6](https://github.com/Yxniy/morphrpc/pull/6) adds a Python standard-library analyzer. It requires no installed FastAPI app or compiler to inspect source.
- The integration CI PR combines these heads so CI can exercise them together. Its diff includes the four implementation branches until they land on `main`. Rebase or otherwise refresh it against the actual merged `main`, verify that its remaining diff contains the intended CI, integration tests, and documentation, and rerun checks before merging it.

## CI and integration evidence

The integration branch adds explicit compiler and generated-project formatting, build, Clippy, and test commands. Dependency audit checks the root, fuzz, compiler, and generated-project lockfiles with the pinned CI audit tool. A separate Python 3.11.11 job runs migration unit tests. The Python runtime action is pinned by commit and the repository token is read-only. The Rust and Python checks run for pull requests, pushes to `main`, and merge-queue events. The audit also runs weekly. Benchmark reports remain advisory; they should not be configured as required merge checks.

The checked-in integration fixture derives field candidates from the static FastAPI example, keeps its authentication, side-effect, and operation-ID review blockers visible, and compares them with a manually authored test contract. The generated-project test compiles the resulting contract's bindings and sends repeated calls through the v1 TCP runtime. The fixture's operation ID is a test assignment and is not approval to migrate or shadow the example.

On the combined branch based on `main` at `f875968`, local formatting, Clippy, root workspace tests, compiler and generated-project build/tests, eleven Python analyzer and handoff tests, eight Performance Lab report tests, actionlint, and ShellCheck passed. A local RustSec audit found no known vulnerabilities in the four lockfiles. GitHub Actions must pass on the final integration PR head as well. The integration branch initially used PR #4 head `9d3d57b`; the Performance Lab owner was still validating a newer profiling-workflow revision. Refresh and retest that dependency when its head settles.

## Compatibility and remaining risks

The compiler schema version selects an opt-in application-payload convention. It does not negotiate peer schemas or alter the v1 frame. Existing v1 `get_user` payloads are not automatically compatible with generated payloads. Ordered required fields, method/operation-ID changes, and field additions or removals can break compatibility. Generated handlers currently expose generic wire errors, so an adapter must explicitly map application errors and authorization semantics. FastAPI analysis is partial and cannot prove authentication, validation, side effects, or shadow safety.

The root `src/protocol/mod.rs` is unchanged from the inspected `main`. The Performance Lab's client change removes duplicate encoding while retaining the v1 codec and pre-I/O validation; its golden-frame and transport tests passed in the combined local run. Performance Lab timing was collected on a shared host during concurrent development, so its performance conclusions need the documented repeated, controlled review.

At review time GitHub reported that `main` had no active branch protection or rulesets. CI jobs fail on correctness failures, but repository administrators must configure `Rust`, `Dependency audit`, `Workflow lint`, and `Migration assessment` as required checks to enforce them. Benchmark jobs should remain advisory. A PR check can pass before these rules are activated; confirm repository settings separately.

## Recommended merge sequence

1. Review and merge PR #3 to establish the shared contract.
2. After PR #4's owner finishes and all required checks pass, review and merge PR #4. PR #2's head is already an ancestor of PR #4, so do not merge PR #2 independently.
3. Before merging PRs #5 and #6, confirm the integration CI PR has passed against their exact current heads and the chosen PR #4 head. Review and merge PR #5, then PR #6. Until the CI integration PR lands, their standalone checks must be verified on the integration PR or run manually.
4. Refresh the integration CI branch against the resulting latest `main`. Inspect the final diff, rerun all required checks, and merge the integration CI PR immediately after #6. Require the four correctness and audit checks through repository rules once they exist on `main`.

No merge is performed by this report. If PR #4's Linux profiling validation is failing or a head changes, treat that as a blocker for the affected review and rerun the combined checks against the final heads.
