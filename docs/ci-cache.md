# CI cache strategy

The consolidated `cargo-test` job uses [`Swatinem/rust-cache@v2`](https://github.com/Swatinem/rust-cache)
instead of a hand-rolled `actions/cache` entry over `target/`.

## Rationale

- **Warm runs:** rust-cache splits registry, git, and target artifacts with smarter keys.
- **Maintenance:** one action vs manual path lists and lockfile-only keys.
- **Failure retention:** `cache-on-failure: true` keeps partial builds after red CI.

## When to revisit

If CI time regresses, compare a warm run before/after in Actions job timing.
Revisit if workspace grows multiple independent target graphs.
