# CLAUDE.md — Viva Studio

Viva Studio is a Tauri v2 + React 19 GUI for GenICam cameras. It consumes
viva-service / viva-service-u3v over Zenoh and is the **second Cargo
workspace** of this monorepo (kept separate from the published library
workspace at the repo root — see ADR-0017).

## Pointers

- Root `CLAUDE.md` — repo-wide rules (pre-push gates, version bumps).
- `docs/design.md` — overall architecture; `docs/backlog.md` — ST epic.
- `docs/studio/` — Zenoh API contract, camera-service API, testing
  cookbook, manual E2E checklist.

## Build & Run

```bash
# Rust workspace (from studio/)
cargo build && cargo test --workspace

# UI dev server
cd studio/ui/viva-studio-ui && bun install && bun run dev

# Tauri dev
cd studio/apps/viva-studio-tauri && cargo tauri dev

# E2E against the in-repo fake camera (3 terminals, repo root)
# T1: cargo run -p viva-fake-gige
# T2: cargo run -p viva-service -- --iface lo0 --zenoh-config studio/config/zenoh-local.json5
# T3: cd studio/apps/viva-studio-tauri && cargo tauri dev
```

## Key Invariants

- Zenoh payload types come ONLY from `viva-zenoh-api` (which must not
  depend on zenoh itself).
- XML parsing happens only in Rust crates (`viva_xml_model` on top of
  `viva-genapi-xml`). The UI consumes UiGraph JSON and never parses XML.
- UiGraph contract fields `nodes_by_name` / `categories` / `root_category`
  are frozen.
- Unknown XML nodes are never dropped: preserved as `UiNodeKind::Unknown`
  with `RawNode` populated.
- In the Tauri backend use `tauri::async_runtime::spawn`, never bare
  `tokio::spawn`.
- Package manager is bun, never npm (`bun.lock` is authoritative).
- `apps/` stays thin glue (windowing, dialogs, command wiring) — no
  parsing or business logic.
