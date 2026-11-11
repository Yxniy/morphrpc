# Contributing to MorphRPC

For a bug report, include the affected revision, Rust version, operating system, and steps to reproduce it. For a protocol or architecture change, open an issue describing the use case, compatibility impact, and trade-offs so contributors can discuss the design.

## Pull requests

1. Fork the repository and create a branch from `main`.
2. Keep the change focused. Include tests for changed behavior and update relevant documentation.
3. Run the relevant local checks from the [development and CI guide](docs/development.md). Check the PR's Actions results and address failures.
4. Describe the change, how you verified it, and any effect on security, the wire format, or service compatibility. For performance changes, include reproduction commands and measured results from a comparable baseline.
5. Get independent approval after the required checks pass. Coding agents must follow [AGENTS.md](AGENTS.md) when submitting and reviewing changes.

Review generated code as you would handwritten code. Include generator inputs and reproduction steps when they are needed to verify the change.

Use [GitHub private vulnerability reporting](https://github.com/Yxniy/morphrpc/security/advisories/new) to report suspected security vulnerabilities. Follow the [code of conduct](CODE_OF_CONDUCT.md) in project spaces.
