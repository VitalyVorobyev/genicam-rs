<p align="center">
  <img src="assets/viva-genicam-logo-and-text_opt.svg" alt="viva genicam" width="360">
</p>

<p align="center">
  Pure Rust building blocks for <b>GenICam</b>: <b>GigE Vision</b> and <b>USB3 Vision</b>.
</p>

<p align="center">
  <a href="https://github.com/VitalyVorobyev/viva-genicam/actions/workflows/ci.yml"><img src="https://github.com/VitalyVorobyev/viva-genicam/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://crates.io/crates/viva-genicam"><img src="https://img.shields.io/crates/v/viva-genicam.svg" alt="crates.io"></a>
  <a href="https://docs.rs/viva-genicam"><img src="https://docs.rs/viva-genicam/badge.svg" alt="docs.rs"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
</p>

> **Status -- not production-ready.** Under active development, pre-1.0, API
> changes between releases. The protocols are implemented and well tested
> against simulators and 35 real vendor XML descriptions, but barely tested
> against physical cameras -- we have none. We are working towards production
> readiness one reported camera at a time, and **reports are genuinely welcome**.
> See [Project status](#project-status--not-production-ready).

> **Disclaimer** -- Independent open-source Rust implementation of GenICam-related standards.
> Not affiliated with, endorsed by, or the reference implementation of EMVA GenICam.
> GenICam is a trademark of EMVA.

---

## Features

- **Discovery** -- find GigE Vision cameras via GVCP broadcast; USB3 Vision cameras via USB enumeration
- **Control** -- read and write device registers and GenApi features (Integer, Float, Enum, Boolean, Command, String, SwissKnife, Converter)
- **Streaming** -- GigE: GVSP reassembly with backpressure (packet resend is implemented but not yet wired into the receive path); U3V: async frame iterator over USB bulk reads
- **IP configuration** -- FORCEIP for temporary assignment; persistent IP registers for permanent configuration
- **Events & actions** -- subscribe to camera events; trigger synchronized acquisition via action commands
- **Time & chunks** -- map device timestamps to host time; parse chunk data (timestamp, exposure, gain)
- **Service bridge** -- expose cameras over [Zenoh](https://zenoh.io/) for Viva Studio (the desktop GUI in `studio/`)
- **No hardware required** -- built-in fake cameras (`viva-fake-gige`, `viva-fake-u3v`) for testing and demos

## Project status — not production-ready

**Read this before depending on the project.** viva-genicam is under active
development and is not yet suitable for production use. It is pre-1.0, the
API changes between releases, and the odds that an arbitrary camera works
first time are not yet good enough to build a product on.

We intend to get it there. What follows is where it actually stands.

**What is implemented:**

- GigE Vision — discovery, control, streaming, events, actions, chunks, IP configuration
- USB3 Vision — discovery, control, streaming, service bridge, CLI
- GenApi — Tier-1 and Tier-2 nodes, `pValue` delegation, the GenICam formula language, the register address model

**What that is backed by:** 278 automated tests, in-process fake cameras for
both transports, and a conformance corpus of 35 real vendor GenApi XML
descriptions (AVT, Basler, Baumer, FLIR, Hikrobot, JAI, PCO, Point Grey,
Photonic Science, Prosilica, Sony, SVS, TIS).

**The honest limitation:** almost none of this has been exercised against
physical hardware. The maintainer has no GigE Vision or USB3 Vision cameras.
Fake cameras only reproduce the behaviour we already thought of, so they
confirm that our assumptions are self-consistent — not that they are right.
Every camera-specific bug found so far was found by a user, not by us.

That has a visible track record. Three reports, three real defects:

| Report | Cause | Outcome |
|---|---|---|
| [#45](https://github.com/VitalyVorobyev/viva-genicam/issues/45) FLIR Blackfly S | XML entity unescaping over CDATA; then the `=` equality operator | Fixed; the reporter's XML is now a permanent test fixture |
| [#35](https://github.com/VitalyVorobyev/viva-genicam/issues/35) Hikrobot MV-CS050-10GC | Register addressing and the formula language, ten defects in one audit | Fixed; the reporter's XML is in the corpus, and the audit became [ADR-0018](docs/adrs/adr0018-genapi-conformance-over-convenience.md) |
| [#57](https://github.com/VitalyVorobyev/viva-genicam/issues/57) JAI FS-3200T on Windows | Link-local addresses dropped; MAC parsed two bytes off | Fixed; the reporter supplied protocol evidence and a patch |

In each case one vendor construct made a camera impossible to open, and in
each case the fix generalised well beyond the camera that reported it.

**So: if your camera does not work, that is a bug worth reporting, and we
want the report.** Cameras deviate from the standard, contradict their own
documentation, and behave inconsistently across firmware revisions — working
with the hardware that exists, rather than the hardware the specification
describes, is the goal rather than a compromise. The most useful thing you
can attach is your camera's GenApi XML: it becomes a permanent regression
fixture, which is how this class of bug stops recurring for everyone with
that model. See the
[issue templates](https://github.com/VitalyVorobyev/viva-genicam/issues/new/choose).

**Known gaps**, tracked in [docs/backlog.md](docs/backlog.md) and
[docs/roadmap.md](docs/roadmap.md):

- GVSP packet resend is implemented at the protocol layer but not wired into the receive path
- The GVCP/GVSP layer is mid-audit. `ACTION` and `EVENT` are fixed, but chunk delivery still is not: the leader rejects the Image Extended Chunk payload type, and the chunk trailer layout matches our own fake rather than the specification
- Several GenApi features are parsed but not yet honoured (`pInvalidator`, `Cachable`, dynamic `pMin`/`pMax`)
- No feature-matrix or MSRV enforcement in CI

## Workspace layout

```
crates/
  viva-gencp/          GenCP encode/decode (transport-agnostic)
  viva-gige/           GigE Vision transport (GVCP/GVSP)
  viva-u3v/            USB3 Vision transport
  viva-genapi-xml/     GenICam XML parser
  viva-genapi/         GenApi node map & evaluation engine
  viva-genicam/        Public API facade (start here)
  viva-pfnc/           Pixel Format Naming Convention tables
  viva-sfnc/           Standard Feature Naming Convention constants
  viva-zenoh-api/      Shared Zenoh wire types (no Zenoh dependency)
  viva-service/        Zenoh bridge: GigE cameras -> Viva Studio
  viva-service-u3v/    Zenoh bridge: U3V cameras -> Viva Studio
  viva-camctl/         CLI binary
  viva-fake-gige/      Fake GigE camera for testing
  viva-fake-u3v/       Fake U3V camera for testing
```

## Viva Studio (GUI)

The desktop app for GenICam cameras lives in `studio/` as a separate Cargo
workspace in this repository. It is a React 19 + Tauri v2 application that
talks to `viva-service` / `viva-service-u3v` over Zenoh, so the published
library crates stay decoupled from GUI and Node toolchain concerns. See
`studio/CLAUDE.md` for build commands and invariants, and `docs/studio/`
for the Zenoh API contract and testing cookbook.

## Quick start

```bash
cargo add viva-genicam
```

```rust
use viva_genicam::{gige, Camera};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Discover cameras on the network
    let devices = gige::discover(Duration::from_secs(1)).await?;
    println!("Found {} cameras", devices.len());

    // Connect to the first camera
    let mut camera = viva_genicam::connect_gige(&devices[0]).await?;

    // Read and write features
    let exposure = camera.get("ExposureTime")?;
    println!("ExposureTime = {exposure}");
    camera.set("ExposureTime", "5000")?;
    Ok(())
}
```

## Python

Pre-built wheels are published on PyPI — no C toolchain needed:

```bash
pip install viva-genicam
```

```python
import viva_genicam as vg

cams = vg.discover(timeout_ms=500)
cam = vg.connect_gige(cams[0])
cam.set_exposure_time_us(10_000.0)

with cam.stream() as frames:
    for frame in frames:
        arr = frame.to_numpy()           # NumPy (H, W) or (H, W, 3) uint8
        break
```

See [`book/src/python.md`](book/src/python.md) for the full Python API.

## Documentation

- **[GenICam standards introduction](docs/standards.md)** -- what GenApi, GenCP, GVCP, SFNC, and PFNC are and how they map to crates
- **[Book (mdBook)](https://vitalyvorobyev.github.io/viva-genicam/)** -- tutorials, architecture, networking cookbook
- **[API reference (docs.rs)](https://docs.rs/viva-genicam)** -- generated Rust API docs
- **[Examples](crates/viva-genicam/examples/)** -- 17 runnable examples covering discovery, streaming, events, chunks, and more
- **[System design](docs/design.md)** -- architecture, key abstractions, data flows, design tenets
- **[Decision records](docs/adrs/)** -- why the big choices were made
- **[Roadmap](docs/roadmap.md)** -- what's planned

## Prerequisites

- Rust 1.88+ (edition 2024)
- Windows / Linux / macOS
- Network: allow UDP broadcast on your capture NIC for discovery. Optional: jumbo frames for high throughput.

## Build & test

```bash
cargo build --workspace
cargo test --workspace
cargo doc --workspace --all-features --no-deps
```

## Run examples

```bash
# Discover cameras
cargo run -p viva-genicam --example list_cameras

# Read/write features
cargo run -p viva-genicam --example get_set_feature

# Grab frames
cargo run -p viva-genicam --example grab_gige

# Zero-hardware demo (uses built-in fake camera)
cargo run -p viva-genicam --example demo_fake_camera
```

## viva-camctl CLI

```bash
# Discover GigE Vision cameras
cargo run -p viva-camctl -- list

# Read a feature
cargo run -p viva-camctl -- get --ip 192.168.0.10 --name ExposureTime

# Write a feature
cargo run -p viva-camctl -- set --ip 192.168.0.10 --name ExposureTime --value 5000

# Stream frames with auto packet-size negotiation
cargo run -p viva-camctl -- stream --ip 192.168.0.10 --iface 192.168.0.5 --auto --save 2

# Sustained streaming benchmark
cargo run -p viva-camctl -- bench --ip 192.168.0.10 --duration-s 60 --json-out bench.json

# Assign temporary IP via FORCEIP
cargo run -p viva-camctl -- set-ip --mac DE:AD:BE:EF:CA:FE --ip 192.168.1.100 --force

# Configure persistent IP
cargo run -p viva-camctl -- set-ip --mac DE:AD:BE:EF:CA:FE --ip 192.168.1.100

# USB3 Vision: discover, read/write features, stream
cargo run -p viva-camctl -- list-usb
cargo run -p viva-camctl -- stream-usb --save 3
```

## viva-service (Zenoh bridge)

```bash
# Start the GigE Vision service
cargo run -p viva-service -- --iface en0

# Start the USB3 Vision service (real camera)
cargo run -p viva-service-u3v

# Start the USB3 Vision service with a fake camera
cargo run -p viva-service-u3v -- --fake
```

The service discovers cameras, publishes device announcements, serves GenICam XML,
handles node read/write, and streams frames over Zenoh for Viva Studio
(the desktop GUI in `studio/`).

## Integration testing

Integration tests use the built-in `viva-fake-gige` camera simulator -- no
external tools or hardware required.

```bash
# All tests (unit + integration + service e2e)
cargo test --workspace

# Run a self-contained demo
cargo run -p viva-genicam --example demo_fake_camera
```

## Troubleshooting

- **No devices found** -- check NIC/interface selection and host firewall (UDP broadcast on port 3956)
- **Frame drops at high FPS** -- enable jumbo frames, raise `SO_RCVBUF`, enable inter-packet delay
- **Windows** -- run as admin, allow UDP in firewall rules

## License

MIT -- see [LICENSE](LICENSE).
