# genicam-rs

Pure Rust building blocks for **GenICam** with an **Ethernet-first (GigE Vision)** focus.  
Cargo workspace, modular crates (GenCP, GVCP/GVSP, GenApi core), and small examples.

## Current status (Nov 2025)

- ✅ **Discovery (GVCP)** on selected NICs; enumerate devices.
- ✅ **Control path (GenCP over GVCP):** read/write device memory; fetch GenICam XML.
- ✅ **GenApi (Tier-1):** basic NodeMap (Integer/Float/Enum/Bool/Command), ranges, access modes.
- ✅ **Selector-based address switching** for common features (e.g., `GainSelector`).
- 🚧 **Streaming (GVSP):** packet reassembly, resend, MTU/packet size & delay, backpressure, stats.
- 🚧 **Events & actions:** message channel events; action commands (synchronization).
- 🚧 **Time mapping & chunks:** device↔host timestamp mapping; chunk data parsing.
- 🔜 USB3 Vision; SwissKnife & advanced GenApi; GenTL producer (.cti).

> See `roadmap.md` for detailed phases and acceptance criteria.

## Workspace layout

crates/
  genicp/        # GenCP encode/decode
  tl-gige/       # GigE Vision (GVCP/GVSP)
  genapi-xml/    # GenICam XML loader & schema-lite parser
  genapi-core/   # NodeMap & evaluation
  genicam/       # Public API facade
examples/        # Small demos (see below)

## Prereqs

- Rust 1.75+ (pinned in `rust-toolchain.toml`)
- Linux/macOS (Windows planned)
- Network:
  - Allow UDP broadcast on your capture NIC for discovery
  - Optional: enable jumbo frames if you plan to test high throughput

## Build & test

```bash
# Build everything
cargo build --workspace

# Run all tests
cargo test --workspace

# Generate docs locally
cargo doc --workspace --no-deps
```

## Run examples

Examples currently live at the workspace root. Run them via the appropriate crate target and adjust paths as needed.

- **Discover devices (GVCP broadcast):**

  ```bash
  cargo run -p tl-gige --example list_cameras --features ""
  ```

- **Fetch XML & print minimal metadata (control path):**

  ```bash
  cargo run -p tl-gige --example get_set_feature --features ""
  ```

- **Grab frames (GVSP):**

  ```bash
  cargo run -p tl-gige --example grab_gige --features ""
  ```

- **Events:**

  ```bash
  cargo run -p tl-gige --example events_gige --features ""
  ```

- **Action command (broadcast):**

  ```bash
  cargo run -p tl-gige --example action_trigger --features ""
  ```

- **Timestamp mapping:**

  ```bash
  cargo run -p tl-gige --example time_sync --features ""
  ```

- **Selectors demo:**

  ```bash
  cargo run -p genicam --example selectors_demo --features ""
  ```

## Troubleshooting

- No devices found: check NIC/interface selection and host firewall (UDP broadcast).
- Drops at high FPS: try jumbo frames, raise `SO_RCVBUF`, and enable packet delay.
- Windows: run as admin, allow UDP in firewall rules; jumbo frames must be enabled per NIC.

## License

MIT — see LICENSE.

## Acknowledgements

Standards: GenICam/GenApi (EMVA/A3), GigE Vision. Thanks to the open-source ecosystem for prior art and inspiration.
