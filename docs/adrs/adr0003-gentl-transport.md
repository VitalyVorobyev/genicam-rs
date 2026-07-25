# ADR-0003: GenTL as sole transport abstraction

**Status:** Accepted
**Date:** 2026-03-06

## Context

Industrial cameras use various transport layers: GigE Vision, USB3 Vision, CoaXPress, CameraLink. Each has vendor-specific SDKs. GenTL (GenICam Transport Layer) is the standardized C API that abstracts all these transports behind a single interface.

## Decision

The camera service uses **GenTL exclusively** as its transport abstraction.

- The service dynamically loads GenTL provider `.cti` files at runtime.
- No direct dependency on Aravis, vendor SDKs, or transport-specific libraries.
- The Rust library wraps the GenTL C API using `libloading` for dynamic loading.
- The service discovers available `.cti` providers on the system and enumerates cameras through them.

## Consequences

- Single abstraction covers GigE Vision, USB3 Vision, CoaXPress, and CameraLink.
- Users must have a GenTL provider installed (most camera vendors ship one).
- No compile-time dependency on camera vendor SDKs.
- Provider discovery path is OS-dependent (`GENICAM_GENTL*_PATH` environment variables).
- Some advanced vendor-specific features may not be accessible through GenTL alone.
