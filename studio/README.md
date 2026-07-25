<p align="center">
  <img src="assets/viva-studio-logo-and-text_opt.svg" alt="viva genicam" width="360">
</p>

# GenICam Studio

GenICam Studio is an early-stage toolkit for parsing GenICam XML and exploring the resulting feature model in a UI (browser or desktop).

This repository is **pre-alpha** and under active development. We try to keep the public Rust APIs and the `UiGraph` JSON contract stable by default, but things may change as coverage expands. See `AGENTS.md` for the working agreement (invariants, boundaries, and Definition of Done).

## Status (pre-alpha)

What works today:

- Parse a subset of GenICam XML into a small `UiGraph` model (Rust + WASM).
- Browse categories + features, search, and inspect/edit draft values in offline mode.
- Preserve unknown nodes and surface them as `Unknown` with `RawNode` debug data.

What is intentionally not implemented yet:

- Live device connection (applying values / executing commands is stubbed behind provider interfaces).
- Full GenICam schema coverage (parsing support is expanded incrementally, fixture-first).

Near-term roadmap:

- Expand parsing coverage while keeping `UiGraph` small and debuggable.
- Add richer diagnostics (warnings/errors) to help users understand unsupported constructs.
- Introduce a live provider for device-backed reads/writes (without moving parsing into the UI).

## What’s in this repo

GenICam Studio is organized as a monorepo with a strict separation of responsibilities:

- **Rust crates (`crates/`)**: parsing + normalization into a small, serializable model (`UiGraph`).
- **WASM adapter (`crates/*_wasm`)**: thin `wasm-bindgen` wrappers so the browser can reuse the Rust parser.
- **React UI (`ui/`)**: renders the `UiGraph` contract and provides a feature browser UX. No XML parsing here.
- **Desktop shell (`apps/`)**: Tauri v2 app that hosts the UI and exposes native commands. No duplicated parsing.
- **Streamer (`apps/`)**: standalone Zenoh → BMP → WebSocket bridge for Mono8 frames.

## Architecture (contract-first)

```
GenICam XML
   |
   v
Rust: crates/genicam_xml_model (streaming parse via quick-xml)
   |
   v
UiGraph (serde)  <——— this is the contract the UI consumes
   |
   +--> Browser: crates/genicam_xml_model_wasm  -> WebWasmProvider  -> React UI
   |
   +--> Desktop: apps/viva-studio-tauri      -> TauriProvider    -> React UI
```

Key invariants:

- **Parsing lives in Rust** (native + WASM reuse the same core crate).
- **UI depends on the JSON contract** (`UiGraph`), not on GenICam tags.
- **Unknown nodes are preserved**: unrecognized XML tags become `Unknown` nodes and keep `RawNode` populated.

## Data model: `UiGraph`

`UiGraph` is the minimal model the UI needs to browse and edit features:

- `nodes_by_name`: map from GenICam `Name` → `UiNode`
- `categories`: map from category name → `UiCategory` (with `features: string[]`)
- `root_category`: name of the chosen root (prefers `"Root"` when present)

Each `UiNode` also includes a lightweight `raw` snapshot:

- `RawNode.tag`: original element tag (e.g. `"Integer"`, `"SwissKnife"`)
- `RawNode.attributes`: element attributes (always includes `Name` when present)
- `RawNode.children_text`: simple child text fields (preserves repeated fields like `pFeature` by joining with `\n`)

The TypeScript copy of the contract lives at `ui/viva-studio-ui/src/xml_model/uigraph.ts`.

## Feature Browser (current UI)

The UI currently supports:

- **Category tree browsing** from `UiGraph.categories`
- **Search** by node name or display name
- **Offline editors** for basic node kinds (draft values + validation in the UI)
- **Unknown-node visibility** (toggle to hide/show; unknown nodes render a debug view of `RawNode`)
- **Raw XML** and **model debug JSON** panels for quick inspection

## Quickstart

### Prerequisites

- Rust (stable, `rust-toolchain.toml` pins `rustfmt` + `clippy`)
- Node.js (recommended: 20+)
- `wasm-pack` (for browser/WASM parsing): `cargo install wasm-pack` (or `brew install wasm-pack` on macOS)
- Tauri CLI v2 (for the desktop shell): `cargo install tauri-cli --version '^2'`

### 1) Run the parser tests (Rust)

```sh
cargo test
```

### 2) Run the UI in browser mode (Vite + WASM)

```sh
cd ui/viva-studio-ui
bun install
bun run wasm:build
bun run dev
```

Then load the sample XML fixture:
- `crates/genicam_xml_model/fixtures/minimal.xml`

### 3) Run the desktop app (Tauri v2)

```sh
cd ui/viva-studio-ui
bun install
```

```sh
cargo install tauri-cli --version '^2'
cd apps/viva-studio-tauri
cargo tauri dev
```

Notes:
- `apps/viva-studio-tauri/src-tauri/tauri.conf.json` points Tauri to the Vite dev server at `http://localhost:5183`.
- The Tauri dev workflow starts the UI dev server via `beforeDevCommand`.

## Building (production)

### Build the UI bundle

```sh
cd ui/viva-studio-ui
bun install
bun run build
```

### Build the desktop app

```sh
cd apps/viva-studio-tauri
cargo tauri build
```

## Using as a library (Rust)

`crates/genicam_xml_model` is a regular Rust library crate and can be consumed independently:

```rust
use genicam_xml_model::parse_genicam_xml;

let xml = std::fs::read_to_string("camera.xml")?;
let graph = parse_genicam_xml(&xml)?;
println!("root category: {}", graph.root_category);
```

## WebSocket Streamer (Zenoh → BMP → WebSocket)

The streamer is a small standalone process that:

- Subscribes to a Zenoh key that publishes **tightly-packed Mono8** frames (`width * height` bytes)
- Encodes each frame as an 8‑bit BMP (grayscale palette)
- Broadcasts the latest BMP to all WebSocket clients (latest-only, bounded memory)

### Run

```sh
cargo run -p genicam-ws-streamer -- \
  --image-key quiss/sensors/svcA/devices/cam0/image \
  --width 640 \
  --height 480
```

Optional flags:

- `--bind 127.0.0.1:8081` (default)
- `--path /ws` (default)
- `--fps-limit 30` (drop frames above this rate)
- `--zenoh-config <JSON5|FILE>` (inline JSON5 string or a config file path)

### Quick test with `websocat`

The server sends a small JSON info message first, then binary BMP frames.
To verify the BMP stream quickly:

```sh
websocat ws://127.0.0.1:8081/ws --binary | head -c 2
```

You should see `BM` once the first frame arrives.

## Code map (modules + components)

### Rust

- `crates/genicam_xml_model`
  - `crates/genicam_xml_model/src/model.rs`: `UiGraph` + `UiNode` + `RawNode` (serde contract)
  - `crates/genicam_xml_model/src/parser.rs`: streaming XML parser (builds the contract, preserves unknowns)
  - `crates/genicam_xml_model/src/error.rs`: `ParseError` with context
  - `crates/genicam_xml_model/fixtures/`: small synthetic XML fixtures + JSON snapshots
- `crates/genicam_xml_model_wasm`
  - `crates/genicam_xml_model_wasm/src/lib.rs`: `wasm-bindgen` wrapper that returns a JS-friendly `UiGraph`

### UI (React)

- Contract + providers:
  - `ui/viva-studio-ui/src/xml_model/uigraph.ts`: TypeScript mirror of `UiGraph`
  - `ui/viva-studio-ui/src/xml_model/provider.ts`: `WebWasmProvider` vs `TauriProvider`
  - `ui/viva-studio-ui/src/xml_model/helpers.ts`: tiny pure helpers (labels, unknown checks)
  - `ui/viva-studio-ui/src/xml_model/values.ts`: `NodeValue` draft value union type
  - `ui/viva-studio-ui/src/xml_model/validate.ts`: draft value validation (offline UX)
  - `ui/viva-studio-ui/src/state/useDraftValues.ts`: draft store + validation errors
  - `ui/viva-studio-ui/src/tauri.ts`: environment detection + small native helpers
- Feature Browser components:
  - `ui/viva-studio-ui/src/components/FeatureBrowser/FeatureBrowserPage.tsx`: loads XML, owns filters, selects provider
  - `ui/viva-studio-ui/src/components/FeatureBrowser/CategoryTree.tsx`: root category tree shell
  - `ui/viva-studio-ui/src/components/FeatureBrowser/CategoryTreeNode.tsx`: recursive renderer (cycle-safe)
  - `ui/viva-studio-ui/src/components/FeatureBrowser/FeaturePanel.tsx`: details + editor tabs
  - `ui/viva-studio-ui/src/components/FeatureBrowser/editors/*`: small editors per node kind
  - `ui/viva-studio-ui/src/styles.css`: global styles for the Feature Browser UI

### Desktop (Tauri)

- `apps/viva-studio-tauri/src-tauri/src/commands/xml_model.rs`: native parse + fixture commands (returns the same `ParseXmlResponse` shape as WASM)
- `apps/viva-studio-tauri/src-tauri/src/state/model_state.rs`: stores the last loaded model to avoid re-parsing

### Streamer (standalone)

- `apps/genicam-ws-streamer/src/main.rs`: CLI + wiring + shutdown
- `apps/genicam-ws-streamer/src/bmp.rs`: Mono8 BMP encoder + tests
- `apps/genicam-ws-streamer/src/zenoh_source.rs`: Zenoh subscriber loop + FPS drop + watch channel
- `apps/genicam-ws-streamer/src/ws.rs`: WebSocket server + fan-out

## Scripts and CI

- `scripts/wasm-build.sh`: builds the WASM adapter into the UI’s import location (used by CI).
- GitHub Actions workflow: `.github/workflows/ci.yml` runs `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, builds WASM, and builds the UI.

## Fixtures and snapshots

Fixtures live under `crates/genicam_xml_model/fixtures/` and are intentionally small and synthetic.

- `minimal.xml` is a smoke fixture with at least one **unknown node** to keep preservation behavior covered.
- `expected_minimal.json` is a pretty-printed snapshot of the serialized `UiGraph`.

When you change parsing/model behavior:

1. Update or add fixtures.
2. Update the corresponding Rust tests under `crates/genicam_xml_model/tests/`.
3. Intentionally refresh snapshot JSON (if it changed).

## Contributing (guiding rules)

This repo is early-stage, but aims to stay clean and contract-first:

- Avoid breaking changes to public Rust APIs and the `UiGraph` JSON shape unless explicitly planned.
- Keep parsing/normalization in Rust crates; keep the UI contract-only.
- Preserve unknown tags as `Unknown` nodes and keep `RawNode` populated.
- Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` for changed crates.

## License

MIT (see `LICENSE`).
