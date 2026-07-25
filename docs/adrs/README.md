# Architecture Decision Records

Decisions that shaped viva-genicam, in the classical ADR format
(Context / Decision / Consequences). Add a new ADR whenever an architectural
decision is made — retrospective ADRs for past decisions are welcome too.

Numbers **0001–0010** are the ADRs imported from genicam-studio
(its ADR-001..010; references such as "ADR-010" in CHANGELOG.md remain
valid). ADR-010 was missing from the studio's own index; it is included
here.

| ADR | Title | Status |
|-----|-------|--------|
| [0001](adr0001-desktop-primary.md) | Desktop-primary with WASM maintenance mode | Accepted |
| [0002](adr0002-camera-service-architecture.md) | Camera service as library + Zenoh process (external) | Accepted |
| [0003](adr0003-gentl-transport.md) | GenTL as sole transport abstraction | Accepted |
| [0004](adr0004-single-camera-scope.md) | Single-camera connection model | Accepted |
| [0005](adr0005-pixel-format-support.md) | Full SFNC pixel format coverage | Accepted |
| [0006](adr0006-progressive-disclosure-ui.md) | Progressive disclosure for Image Viewer controls | Accepted |
| [0007](adr0007-configurable-sfnc-groups.md) | Configurable SFNC feature groups in Image Viewer | Accepted |
| [0008](adr0008-zenoh-api-contract.md) | Zenoh key-expression API contract | Accepted |
| [0009](adr0009-uigraph-json-contract.md) | UiGraph as the single UI data contract | Accepted |
| [0010](adr0010-feature-state-contract.md) | FeatureState as the authoritative live-state contract | Accepted |
| [0011](adr0011-pure-rust-genicam-stack.md) | Pure-Rust GenICam Stack | Accepted |
| [0012](adr0012-layered-crate-architecture.md) | Layered Crate Architecture with a Single Workspace Version | Accepted |
| [0013](adr0013-fake-camera-first-testing.md) | Fake-Camera-First Testing and the Realism Policy | Accepted |
| [0014](adr0014-sync-registerio-async-adapters.md) | Synchronous RegisterIo with Async Adapters | Accepted |
| [0015](adr0015-vendored-libusb-lgpl-notices.md) | Vendored libusb in PyPI Wheels with LGPL Notices | Accepted |
| [0016](adr0016-cargo-deny-single-gate.md) | cargo-deny as the Single Supply-Chain Gate | Accepted |
| [0017](adr0017-studio-monorepo-two-workspaces.md) | Studio Monorepo with Two Cargo Workspaces | Accepted |

**Template:** `# ADR-NNNN: Title`, `**Status:**`, `**Date:**`, `## Context`,
`## Decision`, `## Consequences` (Positive/Negative). File name:
`adrNNNN-kebab-title.md`.
