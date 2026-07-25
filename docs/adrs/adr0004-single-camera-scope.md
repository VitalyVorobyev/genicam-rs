# ADR-0004: Single-camera connection model

**Status:** Accepted
**Date:** 2026-03-06

## Context

The application could support connecting to multiple cameras simultaneously or limit to one at a time.

## Decision

GenICam Studio supports **one connected camera at a time**. The Zenoh API already uses per-device key namespaces (`genicam/devices/{device_id}/...`), so multi-camera is not precluded at the protocol level, but the UI and application state manage a single active connection.

## Consequences

- Simpler state management: one `ConnectionState`, one `NodeValueCache`, one `AcquisitionInner`.
- Device sidebar shows discovered cameras; connecting to a new one disconnects the previous.
- The Zenoh API contract is already multi-camera capable; future multi-camera support requires only UI and state changes.
- Image Viewer shows a single stream.
