# CodeQL

This repository uses **GitHub default CodeQL setup** (repository Settings →
Code & security → Code scanning).

Do not add a custom `.github/workflows/codeql.yml` with advanced/manual build
while default setup is enabled — SARIF upload fails with:

> CodeQL analyses from advanced configurations cannot be processed when the
> default setup is enabled

Rust analysis runs through the default configuration on `main` and PRs.
