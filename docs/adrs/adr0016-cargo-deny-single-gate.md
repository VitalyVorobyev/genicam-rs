# ADR-0016: cargo-deny as the Single Supply-Chain Gate

**Status:** Accepted
**Date:** 2026-07-25

## Context

Until July 2026, supply-chain checking was split: PR CI ran one view of the
world while the weekly audit job ran cargo-audit with its own advisory
handling and no shared allow-list. The result was six consecutive weeks of
red weekly runs that nobody could act on — the failures weren't reproducible
from a PR, and there was no single place to record "known, accepted, expires
on date X". Meanwhile licenses, banned crates, and registry sources were not
checked at all, which is how the libusb LGPL gap (ADR-0015) went unnoticed.

## Decision

cargo-deny, configured by the repo's `deny.toml`, is the single source of
truth for all four check classes — advisories, licenses, bans, sources — and
the *same* configuration runs in both PR CI and the weekly scheduled audit.
cargo-audit is dropped. Temporary exceptions live only in `deny.toml` as
documented, time-boxed `ignore` entries with a reason and a revisit date.

## Consequences

**Positive:**

- One allow-list to maintain; a green PR and a green weekly run mean the
  same thing, so a red weekly run is actionable by construction.
- License policy is enforced continuously instead of discovered forensically.
- Exceptions are visible in review — editing `deny.toml` is a diff, not a
  CI-dashboard setting.

**Negative:**

- Single-tool blind spots: anything cargo-deny doesn't model isn't checked.
  Accepted for now; further tooling (e.g. cargo-semver-checks, fuzzing) is
  tracked separately in the roadmap.
- The ignore list rots unless the time-boxes are honored; reviewers must
  push back on undated ignores.
