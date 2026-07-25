# Third-Party Notices

The `viva-genicam` project code in this package is licensed under the MIT
license. The **binary wheel** additionally contains statically linked
third-party code with its own license, listed below.

## libusb

- **Version:** 1.0.27
- **Homepage:** <https://libusb.info>
- **Copyright:** © the libusb contributors
- **License:** GNU Lesser General Public License v2.1 or later
  (LGPL-2.1-or-later) -- full text in [`LICENSES/LGPL-2.1.txt`](LICENSES/LGPL-2.1.txt)

The wheel's native extension module (`viva_genicam._native`) statically
links libusb for USB3 Vision support. The libusb sources are vendored
unmodified via the [`libusb1-sys`](https://crates.io/crates/libusb1-sys)
Rust crate (enabled through `rusb`'s `vendored` feature); the upstream
sources are available at <https://github.com/libusb/libusb>.

## Relinking (LGPL-2.1 §6)

As required by section 6 of the LGPL, you may modify libusb and relink
this package's native module against your modified version. The native
module can be rebuilt from source with [maturin](https://maturin.rs)
(`maturin build` in `crates/viva-pygenicam` of the source repository,
<https://github.com/VitalyVorobyev/viva-genicam>). To link against a
modified or system libusb instead of the vendored copy, remove the
`vendored` feature from the `rusb` dependency in
`crates/viva-pygenicam/Cargo.toml` before building; the build then links
whatever libusb `pkg-config` finds on your system.
