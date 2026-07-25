# Roadmap

Mid-term direction, ordered by phase. This file only looks forward — "done"
history lives in [CHANGELOG.md](../CHANGELOG.md). Immediate, actionable tasks
are tracked in [backlog.md](backlog.md).

Phase 1 (July 2026 — CI un-break, Hikrobot READMEM fix #35, external PR #34,
0.2.6 release with LGPL notices, CI modernization) is complete.

## Phase 2 — Streaming reliability (industrial core, next up)

The features that make the library trustworthy on a factory floor.

- **Fallible connect on malformed camera XML** — `NodeMap::from` currently
  `expect`s during connect; a camera serving broken XML must yield an error,
  not a panic.
- **SCPS read-back after write** — cameras clamp the requested packet size;
  the receiver's stride must follow the *negotiated* value read back from
  SCPS, including the extended-ID stride variant.
- **Per-stream ephemeral ports + `source_filter` enforcement** — today
  multiple cameras can cross-talk into one receiver; each stream gets its own
  port and drops packets from unexpected sources.
- **Wire packet resend end-to-end** — `ResendPlanner` and `request_resend`
  exist and are tested but dead; route resend requests through the GVCP
  transaction demux, gated on `resend_enabled`.
- **Library-owned heartbeat keepalive** — consumers currently lose CCP after
  ~3 s idle unless they poll; the library must own the heartbeat.
- **Fix unsound `unsafe impl Sync` on `MockUsbTransfer`** — replace the
  `RefCell` interior with a `Mutex`.

## Phase 3 — 0.3.0 API consolidation (breaking)

One deliberate breaking release to pay down surface-area debt.

- Single frame-reassembly implementation shared by all paths.
- Curated public surfaces: kill blanket `pub mod` in viva-gige, viva-u3v,
  viva-service, viva-camctl, viva-fake-gige; re-export currently unnameable
  public types.
- Error source chains everywhere (no `String` payloads); `#[non_exhaustive]`
  policy for public enums.
- Dedupe viva-service vs viva-service-u3v (~60% copy-paste) behind a
  `StreamSource` trait.
- Fakes import register constants from the transport crates instead of
  redefining them.
- viva-pfnc as the single `PixelFormat` authority.
- Workspace lints: `missing_docs`, `unreachable_pub`.

## Phase 4 — Production infrastructure

- CI `--all-features` job — U3V/USB code is currently never compiled in CI.
- MSRV job.
- Fuzzing for packet and XML parsers (untrusted network input).
- cargo-semver-checks on release tags.
- GenApi: `Cachable` / `pInvalidator` / `PollingTime`, dynamic pMin/pMax
  enforcement.
- GenApi chunk adapter (replace the hardcoded 4-entry chunk table).
- U3V event channel + endpoint-desync recovery.
- Book: production-tuning chapter (rmem, udev, usbfs).
- Align README claims with shipping reality (resend/backpressure are
  currently overstated).

## Studio (after the genicam-studio merge)

- **M10** — real-service integration (studio against viva-service, not mocks).
- **M11** — release prep: DMG/AppImage/MSI packaging, service sidecar.
- **M12** — recording & polish.

Details land in [backlog.md](backlog.md) once the studio import is in.
