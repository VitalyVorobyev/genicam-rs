genicam-rs
===============

Pure Rust building blocks for the GenICam ecosystem — starting with GigE Vision (GVCP/GenCP/GVSP) and paving the way for USB3 Vision. This repo is a Cargo workspace composed of small, focused crates that you can use independently or together.

Status: early work-in-progress. Discovery and control-path primitives are taking shape; streaming and richer GenApi support come next. See roadmap.md for details.

What’s here
- genicp: Transport-agnostic GenCP encode/decode + status mapping.
- tl-gige: GigE Vision transport — GVCP discovery; GenCP over GVCP; device memory read/write.
- genapi-xml: Fetch GenICam XML via device registers (local:address=…); minimal parsing (schema version, top-level features).
- genapi-core: Early Node/NodeMap types to model features (Integer/Float/Enum/Bool/Command/Category).
- genicam: Public façade planned to bridge transports with GenApi nodes.
- Placeholders: tl-u3v, pfnc, sfnc, gentl-cti (scaffolding for later phases).

Workspace layout
- Cargo workspace at the root, with member crates in crates/ and example programs in examples/.
- Rust 1.75 (pinned in rust-toolchain.toml).
- Shared metadata (license, repository) set via [workspace.package] in Cargo.toml.

Quick start
- Build everything: cargo build --workspace
- Run tests: cargo test --workspace
- Generate docs locally: cargo doc --workspace --no-deps

Examples
The repository includes example programs under examples/ to demonstrate current capabilities:

- examples/list_cameras.rs — broadcast GVCP discovery and print IP/MAC/manufacturer/model.
- examples/get_set_feature.rs — connect via GVCP/GenCP, fetch GenICam XML (local:… URL), parse minimal metadata.

Note: examples currently live at the workspace root, not under a specific crate. Until they are moved to per-crate example targets, you can run them by temporarily copying into a crate’s examples/ folder. For instance, to run list_cameras with tl-gige:

1) Copy the file into the crate’s example directory:
   mkdir -p crates/tl-gige/examples && cp examples/list_cameras.rs crates/tl-gige/examples/

2) Run it with cargo:
   cargo run -p tl-gige --example list_cameras --features ""

Alternatively, open the example in your IDE and run it in a small ad-hoc binary crate that depends on tl-gige and tokio.

Minimal usage (snippet)
This is the essence of GigE discovery using tl-gige:

```rust
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let devices = tl_gige::discover(Duration::from_millis(500)).await?;
    for d in devices {
        println!("{} {:?} {:?}", d.ip, d.manufacturer, d.model);
    }
    Ok(())
}
```

Roadmap
- Detailed phases and acceptance criteria live in roadmap.md.

Platform notes
- OS: Linux/macOS are primary targets while developing networking and async I/O. Windows support is planned.
- Networking: Discovery uses UDP broadcast; ensure your firewall permits local broadcasts on your selected interface.

Contributing
- Toolchain: Rust 1.75+ with rustfmt and clippy (installed via rustup; see rust-toolchain.toml).
- Before opening a PR: cargo fmt --all, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace.
- Discussions, bug reports, and PRs are welcome.

License
- MIT — see LICENSE at the repository root.

Acknowledgements
- Specs and terminology from the A3 GenICam, GigE Vision, and USB3 Vision standards.
