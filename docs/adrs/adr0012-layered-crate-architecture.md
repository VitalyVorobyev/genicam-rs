# ADR-0012: Layered Crate Architecture with a Single Workspace Version

**Status:** Accepted
**Date:** 2026-07-25

## Context

The GenICam ecosystem naturally decomposes: protocol primitives (GenCP),
transports (GigE, U3V), XML parsing, the GenApi evaluation engine, a
user-facing facade, and services/bindings on top. We could ship one large
crate, or many crates with independent versions, or many crates on one
shared version.

Consumers pull pieces at different depths: genicam-studio needs
`viva-genapi-xml`/`viva-genapi` (including on wasm) without any transport;
the Python wheel needs the facade; the services need everything.

## Decision

Strict layering across small crates — `viva-gencp` → `viva-gige`/`viva-u3v`
→ `viva-genapi-xml` → `viva-genapi` → `viva-genicam` (facade) → services,
CLI, Python bindings — with a dependency rule of "only downward". All
workspace crates plus the Python package share a single release version
(`[workspace.package] version`), and the release tag must equal the
workspace version (enforced in CI).

## Consequences

**Positive:**

- Independent reuse: the studio consumes the XML/GenApi layers with no
  transport code; `wasm32-unknown-unknown` builds stay possible because the
  upper layers never link sockets or libusb.
- Testability per layer: protocol encode/decode, XML parsing, and node
  evaluation each test in isolation with no hardware.
- Release simplicity: one version to reason about, one CHANGELOG, no
  compatibility matrix between sibling crates.

**Negative:**

- Version churn: an unchanged crate gets republished whenever any sibling
  changes. Accepted — the cost of a compatibility matrix at this stage would
  be far higher than the noise of empty version bumps.
- More manifests to keep consistent; mitigated by `workspace = true`
  inheritance (viva-pygenicam is the one exception and must be bumped by
  hand — see CLAUDE.md "Version Bumps").
