# genicam-rs

Welcome to the genicam-rs workspace. This project provides Rust building blocks
for GenICam-compatible transports, control, and feature access with a focus on
GigE Vision.

## Quickstart

- Install Rust via `rustup` (toolchain pinned in `rust-toolchain.toml`).
- Clone the repository and run `cargo test --workspace`.
- Explore the facade crate with `cargo run -p genicam --example list_cameras`.

## Crates

- `genicp`: GenCP encode/decode primitives.
- `genapi-xml`: XML fetch and minimal parsing helpers.
- `genapi-core`: NodeMap evaluation and feature access.
- `tl-gige`: GigE Vision transport utilities.
- `genicam`: Facade re-export combining the workspace crates.

See the main [README](https://github.com/VitalyVorobyev/genicam-rs/blob/main/README.md)
for status updates and roadmap details.
