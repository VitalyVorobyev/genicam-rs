# viva-genicam

High-level GenICam facade: camera discovery, feature control, image streaming, events, and action commands.

This is the main entry point for the [viva-genicam](https://github.com/VitalyVorobyev/viva-genicam) workspace. It re-exports the lower-level crates and provides convenience wrappers so you can get started with a single dependency.

> **Disclaimer** -- Independent open-source Rust implementation of GenICam-related standards.
> Not affiliated with, endorsed by, or the reference implementation of EMVA GenICam.
> GenICam is a trademark of EMVA.

## Project status — not production-ready

This crate is under active development and is not yet suitable for production
use. It is pre-1.0 and the API changes between releases.

The protocols are implemented against the EMVA specifications and covered by
278 automated tests, in-process fake cameras, and a corpus of 35 real vendor
GenApi XML descriptions — but **almost none of it has been exercised against
physical hardware**, because the maintainer has none. Fake cameras only
reproduce the behaviour we already thought of. Every camera-specific bug found
so far was found by a user.

We intend to make this production-ready, and the way that happens is one
reported camera at a time. Cameras deviate from the standard and contradict
their own documentation; working with the hardware that exists is the goal,
not a compromise. **If your camera does not work, please
[open an issue](https://github.com/VitalyVorobyev/viva-genicam/issues/new/choose)**
— and attach the camera's GenApi XML if you can, since that becomes a
permanent regression fixture for your model.

Current gaps are tracked in
[docs/backlog.md](https://github.com/VitalyVorobyev/viva-genicam/blob/main/docs/backlog.md).

## Features

- **Discovery** -- find GigE Vision cameras on any network interface
- **Connect & control** -- `connect_gige()` one-liner for camera connection with automatic XML fetch
- **Feature access** -- typed get/set for Integer, Float, Enum, Boolean, Command, String features
- **Streaming** -- `FrameStream` async iterator with reassembly and backpressure (packet resend is implemented at the protocol layer but not yet wired into the receive path)
- **Events & actions** -- subscribe to camera events; trigger synchronized acquisition
- **Chunks & timestamps** -- parse chunk data; map device timestamps to host time
- **USB3 Vision** -- optional `u3v` feature for USB3 Vision cameras

## Usage

```bash
cargo add viva-genicam
```

```rust
use viva_genicam::{gige, Camera};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let devices = gige::discover(Duration::from_secs(1)).await?;
    let (mut camera, _xml) = viva_genicam::connect_gige(&devices[0]).await?;
    camera.set("ExposureTime", "5000")?;
    let val = camera.get("ExposureTime")?;
    println!("ExposureTime = {val}");
    Ok(())
}
```

## Feature flags

| Flag | Description |
|------|-------------|
| `u3v` | Enable USB3 Vision transport |
| `u3v-usb` | Enable USB3 Vision with real USB hardware access (includes `u3v`) |

## Documentation

- [API reference (docs.rs)](https://docs.rs/viva-genicam)
- [Book & tutorials](https://vitalyvorobyev.github.io/viva-genicam/)
- [Examples](https://github.com/VitalyVorobyev/viva-genicam/tree/main/crates/viva-genicam/examples)

Part of the [viva-genicam](https://github.com/VitalyVorobyev/viva-genicam) workspace.
