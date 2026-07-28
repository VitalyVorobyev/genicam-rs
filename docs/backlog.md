# Backlog

Immediate, actionable tasks. Mid-term direction lives in
[roadmap.md](roadmap.md); shipped work is recorded in
[CHANGELOG.md](../CHANGELOG.md).

**Legend**

- Priority: **P0** (do next) / **P1** (soon) / **P2** (when convenient)
- Size: **S** (hours) / **M** (a day or two) / **L** (several days) /
  **XL** (a week+)
- Status: `planned` / `in-progress` / `done` / `blocked`

## SR — Streaming reliability (roadmap Phase 2)

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| SR-01 | Make connect fallible on malformed camera XML (`NodeMap::from` `expect` → error) | P0 | S | planned | Panic reachable from remote input |
| SR-02 | SCPS read-back after write; stride follows negotiated value (incl. extended-ID stride) | P0 | M | planned | Cameras clamp requested packet size |
| SR-03 | Per-stream ephemeral ports + `source_filter` enforcement | P0 | M | planned | Multi-camera cross-talk |
| SR-04 | Wire packet resend end-to-end through the GVCP transaction demux, gated on `resend_enabled` | P1 | L | planned | `ResendPlanner`/`request_resend` tested but dead |
| SR-05 | Library-owned heartbeat keepalive | P1 | M | planned | Consumers lose CCP after ~3 s idle |
| SR-06 | Fix unsound `unsafe impl Sync` on `MockUsbTransfer` (`RefCell` → `Mutex`) | P0 | S | planned | Soundness bug |

## XML — Vendor GenApi XML compatibility

Found by inspection or by the vendor XML corpus
(`scripts/fetch-xml-corpus.sh`), not by a user report. Tracked here rather
than as GitHub issues so the tracker stays user-facing.

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| XML-01 | Support negative `<Address>` (chunk-relative offsets) | P2 | M | planned | Confirmed: `Baumer_HXG20` `ChunkImageLength`; sole `EXPECTED_SKIPS` entry in the corpus test. Needs signed/relative addressing in the data model |
| XML-02 | Fall back to lossy decoding for non-UTF-8 GenApi XML | P2 | S | planned | Unconfirmed. `fetch.rs` `String::from_utf8` is strict, so an `ISO-8859-1` document fails the connect outright |
| XML-03 | Accept uppercase `0X` hex prefix in `parse_u64`/`parse_i64` | P2 | S | planned | Unconfirmed. Hex digits are already case-insensitive, only the prefix is not. No corpus document uses it |

## CI

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| CI-01 | `--all-features` build+test job | P1 | M | planned | viva-u3v `usb` feature needs libusb on runners |
| CI-02 | MSRV job | P2 | S | planned | |
| CI-03 | Scope release.yml / publish-docs.yml permissions to the jobs that need write | P2 | S | planned | |
| CI-04 | Drop hardcoded `sleep 30` in publish-crates.yml | P2 | S | planned | cargo ≥ 1.66 waits for the index |
| CI-05 | python.yml path filter misses viva-gencp and root Cargo.toml/Cargo.lock | P2 | S | planned | |
| CI-06 | Windows wheel is built but never tested | P2 | M | planned | |
| CI-07 | `cargo doc` in ci.yml lacks `--all-features` + `RUSTDOCFLAGS` | P2 | S | planned | |

## DOC — Documentation

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| DOC-01 | Book: fill the three crate pages properly (viva-gencp, viva-gige, viva-genapi) | P2 | M | planned | Renamed from stale names in this PR; content is dated |
| DOC-02 | Book: empty chapters (errors-logging, contributing) and api.md broken links | P2 | M | planned | |
| DOC-03 | Add missing `description` to viva-service-u3v and viva-fake-u3v Cargo.toml | P2 | S | planned | |

## ST — Studio (after the genicam-studio import)

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| ST-01 | Modernize studio crates to edition 2024 + workspace-dep inheritance | P2 | M | planned | |
| ST-02 | Retire apps/viva-mock-service in favor of viva-service + viva-fake-gige | P2 | M | planned | |
| ST-03 | Revive studio e2e against in-repo service/fake (drop aravis-from-source) | P1 | L | planned | |
| ST-04 | Refresh stale studio docs (zenoh-api.md says API_VERSION 1; actual is 2) | P1 | S | done | Done by the monorepo import: docs moved to docs/studio/ with names and API_VERSION corrected |
| ST-05 | Delete stale package-lock.json; bun.lock is authoritative | P2 | S | done | Done by the monorepo import (file not carried over) |
| ST-06 | Release packaging pipeline: DMG (macOS), AppImage (Linux), MSI (Windows) on tag push | P1 | L | planned | From studio backlog RP-01 (M11 Release Preparation) |
| ST-07 | Bundle viva-service binary as Tauri sidecar (auto-start if no external service) | P1 | M | planned | From studio backlog RP-02 (M11); was "genicam-service" pre-rebrand |
| ST-08 | Frame annotation rendering engine (frame ID/timestamp/FPS burned into BMP stream) | P2 | M | planned | From studio backlog FA-01 (M11) |
| ST-09 | Annotation toggle in viewer toolbar (config via watch channel to embedded streamer) | P2 | S | planned | From studio backlog FA-02 (M11) |
| ST-10 | Recording playback engine: load .gsr, play/pause, frame step, speed, seek | P2 | L | planned | From studio backlog REC-03 (M12 Recording & Polish) |
| ST-11 | Recording export to TIFF stack / uncompressed AVI | P2 | M | planned | From studio backlog REC-04 (M12); interop with ImageJ, MATLAB |
| ST-12 | Auto-update via Tauri v2 updater plugin (GitHub Releases as update server) | P2 | M | planned | From studio backlog RP-03 (M12) |
| ST-13 | Studio performance benchmarks in CI (BMP encode, UiGraph parse, Zenoh round-trip) | P2 | M | planned | From studio backlog RP-04 (M12); fail on >10% regression |

## API — 0.3.0 consolidation (roadmap Phase 3, breaking)

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| API-01 | Single frame-reassembly implementation | P1 | L | planned | |
| API-02 | Curated public surfaces: kill blanket `pub mod` (viva-gige, viva-u3v, viva-service, viva-camctl, viva-fake-gige); re-export unnameable public types | P1 | L | planned | |
| API-03 | Error source chains (no `String` payloads); `#[non_exhaustive]` policy | P1 | M | planned | |
| API-04 | Dedupe viva-service vs viva-service-u3v behind a `StreamSource` trait | P1 | L | planned | ~60% copy-paste today |
| API-05 | Fakes import transport-crate register constants; viva-pfnc as single PixelFormat authority; workspace lints (`missing_docs`, `unreachable_pub`) | P2 | M | planned | |
