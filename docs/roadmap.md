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

## Phase 0 — Ship 0.4.0 (immediate)

0.3.1 was tagged on 2026-07-31, once #70 confirmed #72's `#[cfg(windows)]`
acquisition fix against a real camera — the one thing it was gated on, and
the one thing no machine here can test.

The lesson worth carrying forward is the shape of that gate, not its outcome:
a green CI on a platform-conditional fix confirms nothing, and the release
waited on a user with the hardware rather than on our own confidence.

**The follow-up release is 0.4.0, not the 0.3.2 this phase used to name, and
that question is now settled** (REL-07). Removing `StreamBuilder::auto_packet_size`
and the Python `open_stream(auto_packet_size=)` argument, and adding variants to
public enums, are breaking under semver; Cargo reads `^0.3` as any 0.3.x, so a
patch would break dependents on their next `cargo update`. Phase 5 renumbers to
0.5.0 as a consequence — see its heading below. Its contents:

- **TC-17 and TC-20**, which landed after the 0.3.1 tag: the GVSP data trailer
  read at offset 2 of an 8-byte payload, and a Linux link-local discovery
  broadcast that resolved to the host's own address.
- **TC-19** — `viva-fake-gige` declared `ChunkModeActive` as `<Integer>` where
  SFNC and all 23 corpus documents that define it say `<Boolean>`, so the chunk
  path had never run end to end against anything. That is why TC-17 was found
  on a user's camera instead of in CI; there is now an end-to-end chunk test.
- **DX-08 and SR-10** — both surfaced by #70's log rather than by a bug report:
  `viva-camctl stream` refused to run without `--iface`, which is the command
  our own documentation tells reporters to use, and a probed 16114-byte MTU was
  discarded in favour of a hardcoded 1500.
- **SR-11 and DC-01** — an unnamed pixel format was silently sized at one byte
  per pixel, and the Zenoh bridge then truncated the frame to that fiction.
- **GA-09, first cut** — `<Register>` limited to a plain `<Length>`, which
  covers 42 of the corpus's 63 declarations and clears every skipped node in
  seven documents.
- **`#[non_exhaustive]` on the public enums that actually grow** — `Node`,
  `NodeDecl`, `ChunkKind`, `ChunkValue`, `ChunkError` — so the next node type is
  not another breaking release. Only possible in a breaking release, which is
  why it rides along with this one.

Retest status is now settled and should not be restated more favourably than it
is: **#45 and #57 confirmed on 0.3.0; #35 was asked and never answered.**

## Phase 1 — Transport conformance (ADR-0019)

ADR-0018 audited the GenApi layer against the specification and found eight
defects. The same audit had never been run on GVCP/GVSP, and the wire layer
carried the same class of error. **Most of what this phase named is now fixed**
— the list is kept because the pattern is the point, not because the work is
outstanding. Per-item status lives in `backlog.md`'s `TC` section.

- `PENDING_ACK` (0x0089) was handled nowhere, so a camera answering a slow
  WRITEMEM/READMEM with a pending-ack produced a hard failure — and flash
  writes and mode changes are exactly what cameras use it for. *Fixed (TC-01)
  for GVCP; the field width is still unsettled against hardware (TC-12).*
- `ACTION_COMMAND` was defined as 0x0080 — the same opcode as `READREG`.
  *Fixed (TC-02).*
- The event channel keyed on 0x000D, which is not a GVCP opcode at all.
  *Fixed (TC-03).*
- The GVSP data trailer was read at the wrong offset — two bytes of an
  eight-byte payload — so `payload_type` and `size_y` were fed to the chunk
  parser as if they were chunk data. Chunks could not decode on any conforming
  camera, and the frame-error check read the trailer's reserved word while the
  real status word was examined nowhere. Found on real hardware (#70), and
  notable as the one case so far where the fake camera was correct and only the
  client was wrong. *Fixed (TC-17); ships in 0.4.0.*

ACTION and EVENT are now implemented by `viva-fake-gige` (TC-07), so both are
exercised by tests. **Still open in this phase**: TC-04 (spec-derived GVSP and
GenCP fixtures), TC-05 (the payload types cameras actually send), TC-06 (chunk
trailer layout), TC-16 (per-transport status-code types) and TC-19.

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
  (discovery raw bytes, bootstrap registers, XML, environment). *Done —
  `viva-camctl xml` and `viva-camctl report`, both of which work on a camera we
  cannot open, which is the only camera anyone reports.*
- `NodeMap::skipped()` is surfaced through camctl, the Python bindings and
  Studio, rather than only appearing in a log line. *Done for camctl (DX-03);
  Python and Studio still open (DX-05).*
- Discovery reports the fields it currently discards — serial number and
  user-defined name — so users and Studio can identify a camera the way its
  label does. *Done (DX-04).*

The loop keeps paying out, and not only through bug reports: **DX-08 and DX-09
were both found by reading a log a reporter attached for an unrelated reason**,
and DX-08 turned out to be our own diagnostic instruction failing on the first
person we gave it to.

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
  *Done (SR-05).*
- **Honest streaming telemetry** — five `StreamStats` counters are permanently
  zero because nothing calls their recorders, and every GVSP parse error is
  swallowed at `trace` level and counted nowhere.
- **Fix unsound `unsafe impl Sync` on `MockUsbTransfer`** — a soundness bug in
  a type that is `pub` in a published crate. *Done (SR-06); every `unsafe impl`
  is now gone from both workspaces.*
- **Size a frame correctly even when the format is unnamed** — an unknown PFNC
  code reports no size, callers fall back to one byte per pixel, and the Zenoh
  bridge truncates the payload to match (SR-11, DC-01).

## Phase 4 — GenApi conformance, round 2

What ADR-0018 did not reach, ordered by corpus frequency rather than by how
interesting it looks. The counts below were measured against the corpus as it
stood at 35 documents; it now holds **37**, and they have not been re-run. They
are here to rank work, not to be quoted as current — and when one of them starts
carrying an argument, re-measure it first. `<Register>`'s count was wrong by
seven declarations and its `<pLength>` split wrong by a factor of eight, because
a line-based `grep` cannot count elements in the single-line XML that FLIR and
PGR ship.

- `pInvalidator` — **18 502 occurrences across 32 of 35 documents**, entirely
  unparsed. Cache invalidation currently fires only on writes made through the
  NodeMap.
- `Cachable` (2 735 / 32) and `PollingTime` (327 / 28) — unparsed, so every
  readable node is cached until a dependency is written.
- `pSelected` (1 534 / 31) — parsed with the direction inverted relative to the
  standard, which registers invalidation edges backwards.
- `pMin` / `pMax` (911 / 1 288) — parsed, stored, registered as dependencies, and
  never read; range checks use the static limits.
- `ImposedAccessMode` (2 709 / 28), `Streamable` (1 700 / 18), `Slope`
  (632 / 29), `pInc` (333 / 25) — unparsed.
- `<Register>` (**63 / 16**, re-measured 2026-07-31) — the raw-byte base
  register type, still dropped. No longer dropped *silently*: GA-02 moved
  unknown node tags into `XmlModel::skipped`, so the corpus allowlist now sees
  them. Two vendors' hardware and an outside contributor's API request all
  point at the same node, `FileAccessBuffer`. Taking the 42 plain-`<Length>`
  declarations first leaves only 21, concentrated in three vendors.
- GenApi chunk adapter, to replace the hardcoded 4-entry chunk table.

**Also in scope: make the corpus test prove more.** Its `viva-genapi` stage
evaluates every node against `NullIo`, which returns zeros — it demonstrates
that nothing panics, not that any value is correct.

## Not a phase — device classes beyond area-scan

Every assumption in this codebase is an area-scan camera's. On 2026-07-31 a
Micro-Epsilon scanCONTROL 850050 — a laser profile scanner — became the first
non-area-scan device in the corpus, contributed by the same person who added
its `Coord3D_*` pixel formats. The formats now exist in `viva-pfnc`; the device
class does not exist anywhere above it. A `Coord3D_ABC32f` frame reaching the
Zenoh bridge today is truncated to a twelfth of itself and published as valid.

This is deliberately not a numbered phase. The concrete defects are tracked in
`backlog.md`'s `DC` section, and only the one that is verified against code
rather than inferred about hardware is scheduled. The rest wait for the thing
the evidence hierarchy actually values: somebody streaming one of these devices
and telling us what came off the wire.

## Phase 5 — 0.5.0 API consolidation (breaking)

One deliberate breaking release to pay down surface-area debt. It was numbered
0.4.0 until that number was spent on Phase 0 — see there for why the follow-up
to 0.3.1 had to break.

- Typed accessors on `Camera`. Everything currently round-trips through
  `String` even though `NodeMap` one layer down already has
  `get_integer`/`get_float`/`get_bool`/`get_enum`.
- Type-gate the GigE-only methods. `configure_events` and
  `configure_stream_multicast` write GVCP bootstrap registers but are defined
  on the generic `Camera<T>`, so they compile against a U3V camera.
- Single frame-reassembly implementation shared by all paths.
- Curated public surfaces: kill blanket `pub mod`; re-export currently
  unnameable public types; stop exposing node cache internals.
- Error source chains everywhere (no `String` payloads). The
  `#[non_exhaustive]` half of this landed in 0.4.0 for the five enums that
  grow; what remains is deciding the policy for enums added after it.
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
