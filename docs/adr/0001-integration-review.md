# Integration review for ADR 0001

Snapshot of the independent workstreams on November 10, 2026. These pull requests target `main` and require separate review. Their CI and content can change after this snapshot.

- [Shared architecture, PR #3](https://github.com/Yxniy/morphrpc/pull/3), proposes the JSON contract, v1 payload convention, runtime boundary, and analysis-only migration handoff.
- [Compiler, PR #5](https://github.com/Yxniy/morphrpc/pull/5), head `e5a192f`, implements that contract in an independent nested Cargo workspace. Generated codecs and bindings use the existing runtime. Handler error details are deliberately mapped to a generic remote error.
- [FastAPI assessment, PR #6](https://github.com/Yxniy/morphrpc/pull/6), head `88203a2`, statically inventories candidate endpoints and leaves operation IDs and contract naming for human review. It requires neither the compiler package nor application imports.
- [Performance Lab, PR #4](https://github.com/Yxniy/morphrpc/pull/4), head `1aa1a91`, is separately owned and its reported PR checks passed. Its branch contains the work in [CI/security PR #2](https://github.com/Yxniy/morphrpc/pull/2); review their relationship before merging either, rather than integrating both sets of changes independently.

## Integration checks

An isolated, uncommitted integration worktree based on `main` at `f875968` combined the architecture, compiler, migration, and completed Performance Lab branches without textual conflicts. Root formatting, Clippy and workspace tests passed. The standalone compiler's formatting, Clippy and tests passed, including generated-code compilation, malformed and bounded payload tests, and persistent client/server calls. Generated-project Clippy, all ten migration analyzer unit tests, and all eight Performance Lab report tests passed. `cargo-audit` 0.23.0 reported no known vulnerabilities in the compiler and generated-project lockfiles at review time.

A separate test analyzed the example FastAPI file and verified its unresolved authentication, side effects and operation ID. For the test only, a reviewer assigned names and an operation ID and mapped the analyzer's supported field candidates into the ADR contract. The compiler generated Rust from that contract, and a generated client and handler completed a local TCP exchange through the existing v1 runtime. The assignment and test do not establish that the application is safe to migrate or shadow.

The combined test used Performance Lab head `1aa1a91` and its runtime candidate. Its changed paths did not overlap the compiler, migration or ADR paths. Root tests included the v1 golden fixtures and the generated-client TCP exchange used the candidate runtime. All PR #4 checks passed, with the main-only CodeQL job skipped on the PR. Separate worktrees shared the same host, so concurrent compilation may have affected Performance Lab timing runs. Recheck results and CI if any PR head changes.

## Dependencies and recommended review order

1. Review the ADR and its opt-in payload convention, including limits on schema evolution and the distinction between generated payloads and existing application payloads.
2. Review the Performance Lab work and its CI/security predecessor together. Choose an integration path that avoids duplicating PR #2's commits, and rerun checks against the actual `main` base.
3. Add CI coverage for the standalone compiler, its generated project, both associated lockfile audits, and the Python migration tests in coordination with the Performance Lab owner. The current root-workspace Rust check does not discover those nested or Python tests. Local success is not a substitute for automated coverage.
4. Review the compiler against the approved ADR and rerun generated bindings against the final runtime candidate. Review the migration skill's candidate mapping and explicit review boundaries. The analyzer remains independently usable without the compiler.

No workstream was merged into `main` as part of this review. Cross-file or dynamic FastAPI behavior, authentication, errors, side effects, and deployment rollback remain application-specific review items. Contract negotiation and automatic schema evolution remain future design work.
