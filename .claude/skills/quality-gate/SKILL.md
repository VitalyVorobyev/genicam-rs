---
name: quality-gate
description: Run all CI gates locally before pushing (library workspace + studio when touched)
---

# Quality Gate

Run the gates **in order** from the repo root. Stop at the first failure,
report it with the fix hint, and do not continue until it is fixed.

## Library workspace (always)

| # | Command | On failure |
|---|---------|------------|
| 1 | `cargo fmt --all --check` | run `cargo fmt --all`, then re-check |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | fix every warning — CI treats them as errors |
| 3 | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps` | fix doc warnings (broken links, missing docs) |
| 4 | `cargo test --workspace` | show failing test names + output, fix, re-run |

## Studio workspace (only if files under `studio/` changed)

Check with `git diff --name-only main` (or staged changes). If anything
under `studio/` changed, additionally run in `studio/`:

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`

And in `studio/ui/viva-studio-ui`:

4. `bun install --frozen-lockfile && bun run test -- --run && bun run build`

## Notes

- cargo-deny runs in CI (Linux). Run `cargo deny check` locally only if
  `deny.toml` or dependencies changed.
- Transitive feature unification can mask breakage locally — see
  "Pre-Push Checklist" in CLAUDE.md.

## Report

End with a compact table, one row per gate that ran:

```
| Gate                        | Result |
|-----------------------------|--------|
| cargo fmt (root)            | PASS   |
| cargo clippy (root)         | PASS   |
| cargo doc (root)            | PASS   |
| cargo test (root)           | PASS   |
| studio gates                | SKIP (no studio changes) |
```

If all pass: "All quality gates passed." Otherwise list actionable fixes.
