# Roadmap

Mid-term direction, ordered by phase. This file only looks forward — "done"
history lives in [CHANGELOG.md](../CHANGELOG.md). Immediate, actionable tasks
are tracked in [backlog.md](backlog.md).

Phase 1 (July 2026 — CI un-break, Hikrobot READMEM fix #35, external PR #34,
0.2.6 release with LGPL notices, CI modernization) is complete. 0.2.7 and
0.2.8 fixed the XML-load failures behind #45 and #35 and added per-node error
isolation.

**Ordering principle.** [ADR-0018](adrs/adr0018-genapi-conformance-over-convenience.md)
established that priority is argued from measurement, not intuition: count the
construct in the vendor corpus, or point at the user report, before ranking it.
This roadmap applies that rule throughout, which is why several items moved
between phases relative to the previous revision.

## Phase 0 — Ship 0.3.0 (immediate)

0.3.0 is written and version-bumped on `main` but was never tagged; crates.io
and PyPI are still serving 0.2.8. Two of the three open issues are blocked on
that tag, not on code: both #35's and #45's reporters are hitting the
`unexpected '='` panic that 0.3.0 already fixes.

- **Fix #57 — GigE discovery on Windows APIPA networks.** Bundled into 0.3.0
  rather than deferred, because on Windows the library currently discovers
  nothing at all. Four defects reported plus four found alongside them; see
  backlog `REL-01`. Implemented in-repo, crediting the reporter.
- **Tag and publish 0.3.0** (`v0.3.0` + `py-v0.3.0`), then ask #35, #45 and
  #57 to retest.
- **Issue templates** that ask for the artifacts which actually resolved these
  three issues — model, OS, version, `RUST_LOG=debug` trace, raw GenApi XML.

## Phase 1 — Transport conformance (ADR-0019)

ADR-0018 audited the GenApi layer against the specification and found eight
defects. The same audit has never been run on GVCP/GVSP, and the wire layer
turns out to carry the same class of error:

- `PENDING_ACK` (0x0089) is not handled anywhere, so a camera answering a slow
  WRITEMEM/READMEM with a pending-ack produces a hard failure. Flash writes and
  mode changes are exactly what cameras use it for.
- `ACTION_COMMAND` is defined as 0x0080 — the same opcode as `READREG`.
- The event channel keys on 0x000D, which is not a GVCP opcode at all.
- The chunk trailer layout is self-consistent with our fake rather than with
  the standard, and the payload type real cameras use to deliver chunks
  (0x4001) is rejected outright.

Neither ACTION nor EVENT is implemented by `viva-fake-gige`, so neither has
ever been exercised by a test.

**The structural half of this phase matters more than any single fix.**
Issue #57's MAC offset is the *third* time the fake camera and the client have
shared an identical wrong assumption, after the SCPS overhead and the unaligned
READMEM (see [design.md](design.md#testing-strategy)). The realism policy says
fakes must be derived from the spec; nothing enforces it. ADR-0019 adds the
enforcement: **fake-camera wire fixtures are spec-derived byte arrays, asserted
independently of the client parser**, so producer and consumer can no longer
agree on a shared error.

## Phase 2 — Diagnostics loop

The binding constraint on this project is that the maintainer has no hardware:
every fix so far has been diagnosed from an artifact a user supplied. #35 was
solved by a model dump, #45 by the byte offsets in a traceback, #57 by a
reporter who read the Wireshark dissector. Yet there is no supported way to
produce those artifacts — `viva-camctl` has no XML dump, and the Python
retrieval snippet given in #45 had to be retracted and corrected.

- `viva-camctl` gains an XML dump and a single-command diagnostic bundle
  (discovery raw bytes, bootstrap registers, XML, environment).
- `NodeMap::skipped()` is surfaced through camctl, the Python bindings and
  Studio, rather than only appearing in a log line.
- Discovery reports the fields it currently discards — serial number and
  user-defined name — so users and Studio can identify a camera the way its
  label does.

## Phase 3 — Streaming reliability

The features that make the library trustworthy on a factory floor.

- **SCPS read-back after write** — cameras clamp the requested packet size;
  the receiver's stride must follow the *negotiated* value.
- **Per-stream ephemeral ports + `source_filter` enforcement** — the filter is
  configured today but never applied, because the receive path discards the
  packet's source address.
- **Wire packet resend end-to-end** — `ResendPlanner` and `request_resend`
  exist, are tested, and have no production callers. Either wire them or delete
  them; the README currently advertises them as shipping.
- **Library-owned heartbeat keepalive** — consumers lose CCP after ~3 s idle.
- **Honest streaming telemetry** — five `StreamStats` counters are permanently
  zero because nothing calls their recorders, and every GVSP parse error is
  swallowed at `trace` level and counted nowhere.
- **Fix unsound `unsafe impl Sync` on `MockUsbTransfer`** — a soundness bug in
  a type that is `pub` in a published crate.

## Phase 4 — GenApi conformance, round 2

What ADR-0018 did not reach, ordered by corpus frequency rather than by how
interesting it looks. The counts are from the 30-document vendor corpus:

- `pInvalidator` — **11 795 occurrences across 27 of 30 documents**, entirely
  unparsed. Cache invalidation currently fires only on writes made through the
  NodeMap.
- `Cachable` (2 221 / 27) and `PollingTime` (211 / 23) — unparsed, so every
  readable node is cached until a dependency is written.
- `pSelected` (1 012 / 26) — parsed with the direction inverted relative to the
  standard, which registers invalidation edges backwards.
- `pMin` / `pMax` (885 / 583) — parsed, stored, registered as dependencies, and
  never read; range checks use the static limits.
- `ImposedAccessMode` (1 382 / 23), `Streamable` (1 405 / 13), `Slope`
  (437 / 24), `pInc` (175 / 20) — unparsed.
- `<Register>` (35 / 9) — dropped *silently*, because the node-tag gate sits in
  front of the error-isolation path. Unknown node types never reach
  `XmlModel::skipped`, so the corpus test's allowlist can never catch a wholly
  missing node type.
- GenApi chunk adapter, to replace the hardcoded 4-entry chunk table.

**Also in scope: make the corpus test prove more.** Its `viva-genapi` stage
evaluates every node against `NullIo`, which returns zeros — it demonstrates
that nothing panics, not that any value is correct.

## Phase 5 — 0.4.0 API consolidation (breaking)

One deliberate breaking release to pay down surface-area debt.

- Typed accessors on `Camera`. Everything currently round-trips through
  `String` even though `NodeMap` one layer down already has
  `get_integer`/`get_float`/`get_bool`/`get_enum`.
- Type-gate the GigE-only methods. `configure_events` and
  `configure_stream_multicast` write GVCP bootstrap registers but are defined
  on the generic `Camera<T>`, so they compile against a U3V camera.
- Single frame-reassembly implementation shared by all paths.
- Curated public surfaces: kill blanket `pub mod`; re-export currently
  unnameable public types; stop exposing node cache internals.
- Error source chains everywhere (no `String` payloads); `#[non_exhaustive]`
  policy for public enums.
- Dedupe viva-service vs viva-service-u3v behind a `StreamSource` trait.
- Fakes import register constants from the transport crates.
- viva-pfnc as the single `PixelFormat` authority.
- Workspace lints: `missing_docs`, `unreachable_pub`.

## Phase 6 — Production infrastructure

- **Per-crate feature matrix in CI.** `viva-pygenicam` and `studio` are
  excluded from the root workspace, so `cargo test --workspace` never builds
  them; and because sibling crates enable `u3v-usb`, feature unification means
  `viva-genicam`'s *own* default feature set is never verified.
- MSRV job; Windows wheels are published but only tested on Linux and macOS.
- Fuzzing for packet and XML parsers (untrusted network input).
- cargo-semver-checks on release tags.
- Book: production-tuning chapter (rmem, udev, usbfs).
- **Documentation accuracy.** Several book chapters document APIs that never
  existed in this codebase, and the README advertises resend and backpressure
  that have no production callers. This is wrong documentation rather than
  missing documentation, and is tracked as such.

## Services & Studio

- **Announce cadence exceeds Studio's expiry window** — the GigE service
  re-announces roughly every 7 s against a 6 s expiry, so devices can flicker.
- **U3V introspection is typeless** — `U3vDeviceHandle` never overrides
  `get_feature_state`, so every U3V camera reports `kind: "Unknown"` and no
  ranges.
- **U3V service streaming never configures the SIRM** or enables streaming.
- **M10** — real-service integration (studio against viva-service, not mocks).
- **M11** — release prep: DMG/AppImage/MSI packaging, service sidecar.
- **M12** — recording & polish.
