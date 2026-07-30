# API reference

The API reference is generated with `cargo doc` and published alongside this
book. Note that rustdoc uses the **crate** name, with underscores — `viva_gige`,
not `viva-gige`.

## Public API

- [`viva_genicam`](api/viva_genicam/index.html) — the facade. Start here.
- [`viva_genapi`](api/viva_genapi/index.html) — `NodeMap`, node evaluation, `RegisterIo`
- [`viva_genapi_xml`](api/viva_genapi_xml/index.html) — GenICam XML → `XmlModel`
- [`viva_gige`](api/viva_gige/index.html) — GVCP/GVSP transport
- [`viva_u3v`](api/viva_u3v/index.html) — USB3 Vision transport
- [`viva_gencp`](api/viva_gencp/index.html) — GenCP message primitives

## Supporting crates

- [`viva_pfnc`](api/viva_pfnc/index.html) — Pixel Format Naming Convention
- [`viva_sfnc`](api/viva_sfnc/index.html) — Standard Feature Naming Convention
- [`viva_zenoh_api`](api/viva_zenoh_api/index.html) — wire types shared with Viva Studio
- [`viva_camctl`](api/viva_camctl/index.html) — the CLI, as a library
- [`viva_fake_gige`](api/viva_fake_gige/index.html) — the in-process fake camera

The Python API is documented separately in
[Python bindings → API reference](python/api.md); it does not appear in
rustdoc.
