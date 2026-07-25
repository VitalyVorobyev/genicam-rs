# Architecture Decision Records

Decisions that shaped viva-genicam, in the classical ADR format
(Context / Decision / Consequences). Add a new ADR whenever an architectural
decision is made — retrospective ADRs for past decisions are welcome too.

Numbers **0001–0010 are reserved** for the ADRs arriving with the
genicam-studio import (its existing ADR-001..010; references such as
"ADR-010" in CHANGELOG.md remain valid).

| ADR | Title | Status |
|-----|-------|--------|
| 0001–0010 | *Reserved — imported with genicam-studio* | — |
| [0011](adr0011-pure-rust-genicam-stack.md) | Pure-Rust GenICam Stack | Accepted |
| [0012](adr0012-layered-crate-architecture.md) | Layered Crate Architecture with a Single Workspace Version | Accepted |
| [0013](adr0013-fake-camera-first-testing.md) | Fake-Camera-First Testing and the Realism Policy | Accepted |
| [0014](adr0014-sync-registerio-async-adapters.md) | Synchronous RegisterIo with Async Adapters | Accepted |
| [0015](adr0015-vendored-libusb-lgpl-notices.md) | Vendored libusb in PyPI Wheels with LGPL Notices | Accepted |
| [0016](adr0016-cargo-deny-single-gate.md) | cargo-deny as the Single Supply-Chain Gate | Accepted |

**Template:** `# ADR-NNNN: Title`, `**Status:**`, `**Date:**`, `## Context`,
`## Decision`, `## Consequences` (Positive/Negative). File name:
`adrNNNN-kebab-title.md`.
