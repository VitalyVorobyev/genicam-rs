# Changelog

All notable changes to GenICam Studio are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

### Added
- CI-01: End-to-end integration tests — discovery, XML fetch, node set, and bulk read over real Zenoh peer sessions
- ZA-05: API version negotiation — api_version field in DeviceAnnounce; compatibility check on discovery; api-version-mismatch Tauri event
- TB-04: Node value validation in write_node — access mode, type, range, increment, and enum checks before Zenoh send
- ZA-06: NodeValueUpdate extended with optional min/max/inc constraint fields; NodeValueEntry carries runtime constraints to the UI
- FB-05: Export State button exports all live device values as a JSON preset file
- FB-02: Batch apply — Apply All button writes all valid pending drafts sequentially with progress indicator
- FB-01: Live node values shown inline in category tree and search results
- UX-07: Toast notification system for connect/acquire events; skeleton shimmer on XML load
- UX-06: Drag-to-resize splitters for device sidebar and feature browser tree pane, with min/max sizes and localStorage persistence
- UX-04: Feature Browser polish — kind-colored tree icons, search clear button, kind indicator in search results, category row distinction
- UX-03: Device sidebar polish — per-card state dots, connecting animation, animated scan indicator, device count badge
- IV-15: Line profile tool — drag a line on the canvas, floating panel shows per-pixel intensity plot
- IV-14: ROI selection tool — drag to select on canvas, Apply ROI writes Width/Height/OffsetX/OffsetY
- IV-13: Live histogram overlay (grayscale / per-channel RGB) with toolbar toggle
- TB-06: Auto-cleanup and reconnect prompt on unexpected device disconnect
- TB-07: Streamer lifecycle improvement — monitor task with auto-restart (≤5 retries) and streamer-status Tauri event
- ST-02 + IV-17: Multi-format BMP encoder (Mono10-16 downscale, Bayer bilinear debayer, RGB/BGR/RGBa direct) and pixel inspector BMP-aware sampling
- ZA-04: Inline 16-byte frame header (magic, version, format, width, height, seq) prepended to image Zenoh payload; FrameHeader encode/decode in genicam_zenoh_api; mock service prepends, streamer decodes
- ST-03: WebSocket info frame broadcast on meta change; ViewerCanvas propagates pixel_format/dimensions from info frame
- IV-08: Add Color Processing sidebar section (BalanceRatio, Gamma, LUT controls)
- IV-07: Add Auto Functions section (ExposureAuto, GainAuto, BalanceWhiteAuto enum selects) to Image Viewer sidebar
- IV-06: Add Transport Layer section (GevSCPSPacketSize, GevSCPD integer sliders) to Image Viewer sidebar
- IV-05: Add TriggerSection to Image Viewer ControlSidebar (TriggerMode, TriggerSource, TriggerActivation, TriggerDelay)
- ZA-07: Zenoh API spec reviewed; added image/meta section, sequence diagrams, Rust type references, and corrected image payload description
- ST-01: genicam-ws-streamer subscribes to image/meta to self-configure width/height; --width/--height become optional hints (default 640x480)
- IV-16: Add snapshot save (PNG) with native file dialog in Tauri and download fallback in browser mode
- IV-10: Persist sidebar section expand/collapse state per camera model in localStorage
- IV-09: ControlSidebar sections are now driven by sfnc-groups.json order and filtered by UiGraph applicability
- TB-03: Add sfnc-groups.json config file and get_sfnc_groups IPC command with caching; add useSfncGroups React hook
- TB-02: Add read_nodes_bulk IPC command and useNodeBulkRead hook for bulk cache pre-population on connect
- TB-01: Tauri backend subscribes to image/meta and emits image-meta-changed event; UI useImageMeta hook replaces PixelFormat workaround
- UX-05: Image Viewer chrome polish — device name in toolbar, fit-to-window button, clickable zoom badge, snapshot placeholder, fps warning color, distinct status bar surface, idle/active canvas border states
- UX-02: Professional three-zone app header with device-status chip and inline acquisition indicator
- IV-12: Pixel inspector crosshair — hover shows image coordinates and raw pixel value in the status bar
- IV-11 (review fixes): `clampPan` now returns `{0,0}` at fit scale; `--radius-pill` token added; `buildTransform` covered by unit tests
- IV-11: Image Viewer — mouse wheel zoom (cursor-centered), click-drag pan, double-click reset to Fit; zoom level shown in toolbar
- IV-04: Image Format sidebar section — PixelFormat select, Width/Height/OffsetX/OffsetY integer sliders with dynamic OffsetX/Y max, optional BinningHorizontal/BinningVertical selects
- IV-03: Add Exposure & Gain sidebar section with sliders, text inputs, and Auto toggles for ExposureTime, Gain, ExposureAuto, GainAuto
- IV-02: Acquisition control section — Start/Stop button, AcquisitionMode dropdown, and frame counter in Image Viewer sidebar
- IV-01: Image Viewer layout redesign — split canvas/sidebar layout with collapsible control sidebar, toolbar strip, and status bar
- UX-01: Design system — extract CSS tokens to src/styles/tokens.css, complete token vocabulary (warning/danger borders, kind-badge palette, bg-canvas, shadows, z-index scale, focus ring, font weights, transitions)
- **MS-13 / ZA-02**: Bulk node read queryable (`nodes/bulk/read`) — batch-read multiple node values in one Zenoh round-trip; added shared `BulkReadRequest` / `BulkReadResponse` types to `genicam_zenoh_api`
- **MS-12**: Node interdependency simulation (`apps/genicam-mock-service/src/interdependencies.rs`) — Width/Height changes clamp OffsetX/Y and update PayloadSize; PixelFormat change updates PayloadSize
- **MS-10**: Multi-format synthetic image generators in mock service — Mono16 (16-bit little-endian), BayerRG8 (RGGB mosaic), RGB8 (packed) with independent animated channels
- **MS-09**: Image metadata publisher in mock service — publishes `ImageMeta` JSON to `{id}/image/meta` on acquisition start and whenever Width, Height, or PixelFormat changes
- **ZA-01**: `ImageMeta` struct and `image_meta()` key helper in `genicam_zenoh_api`
- **ZA-03**: `PixelFormat` SFNC enum with 27 variants and `bytes_per_pixel()` in `genicam_zenoh_api`

---

## [0.2.0] — Mock Camera Service (M2)

### Added
- **MS-01–08, MS-11**: `apps/genicam-mock-service` — full Zenoh camera simulator
  - Zenoh session, CLI config (`--device-id`, `--width`, `--height`, `--fps`, `--fixture`, `--zenoh-config`)
  - Graceful shutdown on CTRL+C via `tokio::sync::watch` channel
  - Periodic device announcement publisher (`genicam/devices/{id}/announce`)
  - XML queryable serving bundled SFNC fixture (`{id}/xml`)
  - In-memory node store seeded from XML defaults with broadcast change channel
  - `nodes/{name}/set` and `nodes/{name}/execute` queryables with type validation
  - Device status publisher (`{id}/status`)
  - Animated Mono8 gradient generator — brightness reacts to ExposureTime and Gain
  - Acquisition control queryable (Start/Stop) + status publisher
  - Comprehensive SFNC XML fixture (`fixtures/sfnc-standard.xml`) — 7 categories, ~45 nodes including SensorWidth/SensorHeight, ExposureTime, Gain, Width, Height, PixelFormat, TriggerMode, PayloadSize

---

## [0.1.0] — Foundation (M1)

### Added
- GenICam XML streaming parser (`crates/genicam_xml_model`) → `UiGraph` JSON contract
- Feature Browser: category tree, node editors, search, visibility filter, live value display
- WASM adapter (`crates/genicam_xml_model_wasm`) for browser/offline mode
- Tauri v2 desktop shell with IPC bridge
- Zenoh API spec (`docs/zenoh-api.md`) and shared types (`crates/genicam_zenoh_api`)
- Tauri Zenoh backend: device discovery, connect/disconnect, node read/write/execute, acquisition control
- WebSocket streamer (`apps/genicam-ws-streamer`) — Zenoh subscriber → BMP encoder → WS broadcast
- Image Viewer (basic canvas, WebSocket consumer)
- Device sidebar with connection state, diagnostics tab
- Keyboard shortcuts (Ctrl+F search, Escape, Ctrl+Enter apply)
- Window title updates on device connect
