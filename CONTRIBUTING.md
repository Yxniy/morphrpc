# Contributing to MorphRPC

For a bug report, include the affected revision, Rust version, operating system, and steps to reproduce it. For a protocol or architecture change, open an issue describing the use case, compatibility impact, and trade-offs so contributors can discuss the design.

## Pull requests

1. Fork the repository and create a branch from `main`.
2. Keep the change focused. Include tests for changed behavior and update relevant documentation.
3. Run the formatting, Clippy, and test commands in the [README](README.md#development).
4. Describe the change, how you verified it, and any effect on the wire format or service compatibility. For performance changes, include reproduction commands and comparable baseline results.

Review generated code as you would handwritten code. Include generator inputs and reproduction steps when they are needed to verify the change.

Use [GitHub private vulnerability reporting](https://github.com/Yxniy/morphrpc/security/advisories/new) to report suspected security vulnerabilities. Follow the [code of conduct](CODE_OF_CONDUCT.md) in project spaces.
