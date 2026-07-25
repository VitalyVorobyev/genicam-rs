# ADR-0013: Fake-Camera-First Testing and the Realism Policy

**Status:** Accepted
**Date:** 2026-07-25

## Context

GenICam cameras are hardware; CI has none. Options were: mock at the trait
level only, require hardware-in-the-loop rigs, or build full in-process fake
cameras that speak the actual wire protocols (GVCP/GVSP over loopback UDP,
GenCP over a fake USB transport).

We built the fakes (`viva-fake-gige`, `viva-fake-u3v`) and they carry 200+
tests: deterministic, no hardware, no external tools, fast enough for every
push. But in July 2026 two real-world failures exposed a trap:

- **SCPS packet size** — the fake's GVSP sender and our receiver shared the
  same wrong interpretation: both ignored the 36-byte IP+UDP+GVSP header
  overhead. Every streaming test passed because sender and receiver embodied
  the same mistake. External PR #34 exposed it.
- **READMEM alignment** — the fake accepted unaligned READMEM requests that
  real Hikrobot hardware NAKs (issue #35). Tests proved our reader worked
  against our own permissive fake, not against the spec.

In both cases the fake mirrored the implementation's assumptions, so tests
measured self-consistency, not conformance.

## Decision

In-process fake cameras remain the primary test vehicle. Additionally, the
**realism policy**:

1. Fake behavior is derived from the spec text, never from what our own
   client happens to send. Where the spec is strict (GVCP alignment,
   SCPS semantics), the fake is strict — it now enforces GVCP 4-byte
   alignment and serves zipped XML like real devices do.
2. Tests assert payload sizes and content, not just headers/status codes.
3. Predicates and fixtures wire onto real SFNC features; synthetic `Test*`
   nodes are banned from fake-camera XML.
4. Real-hardware reports and external PRs are treated as conformance
   oracles: every such bug becomes a stricter fake behavior plus a test.

## Consequences

**Positive:** hardware-free CI with genuine conformance pressure; the fake
doubles as a demo (`demo_fake_camera`) and a studio E2E backend; regressions
from real cameras become permanently encoded in the fakes.

**Negative:** the fakes are substantial code to maintain, and a spec
misreading can still infect both sides — independent references (real
hardware, aravis interop, external contributors) remain necessary
counterweights.
