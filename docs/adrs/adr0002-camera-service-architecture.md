# ADR-0002: Camera service as library + Zenoh process (external)

**Status:** Accepted
**Date:** 2026-03-06

## Context

GenICam Studio needs to communicate with physical cameras. The architecture assumes an external camera service process that speaks to cameras and exposes data over Zenoh.

## Decision

The camera service is an **external component** that lives in a **separate repository**. It is implemented as a Rust library crate with a thin binary wrapper that runs it as a standalone Zenoh process.

GenICam Studio (this repo) owns:
- The **Zenoh API contract** (`viva-zenoh-api` crate, `docs/zenoh-api.md`) that the service must implement
- The **Tauri desktop app** that consumes the service over Zenoh
- A **mock camera service** (`apps/genicam-mock-service`) for development and testing
- The **camera service API spec** (`docs/camera-service-api.md`) as a contract document

The external camera service owns:
- GenTL interaction, camera SDK calls
- XML retrieval from physical devices
- Node read/write via register access
- Image acquisition and frame streaming
- The Zenoh bridge that maps its internal API to the Zenoh key-expression contract

## Consequences

- Clear repo boundary: GenICam Studio never depends on camera SDKs or GenTL.
- The Zenoh API contract is the single integration point between the two repos.
- The mock service enables full end-to-end development and CI without hardware.
- The API spec in `docs/camera-service-api.md` serves as documentation for the external service implementor.
