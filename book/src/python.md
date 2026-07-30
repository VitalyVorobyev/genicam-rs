# Python bindings

The `viva-genicam` Python package wraps the Rust workspace behind a NumPy-friendly API. It ships as a pre-built wheel on PyPI — no C toolchain, no aravis, libusb is statically bundled.

```bash
pip install viva-genicam
```

The install also provides the `viva-camctl` CLI — see
[Install & hello-camera](python/install.md#the-cli-comes-with-it).

```python
import viva_genicam as vg

cams = vg.discover(timeout_ms=500)
cam = vg.connect_gige(cams[0])
print(cam.get("DeviceModelName"))

with cam.stream() as frames:
    for frame in frames:
        arr = frame.to_numpy()           # NumPy (H, W) or (H, W, 3) uint8
        break
```

## Tutorials

1. [Install & hello-camera](python/install.md) — install the wheel, run the self-contained fake-camera demo.
2. [Discovery](python/discovery.md) — enumerate GigE and U3V cameras, restrict to one NIC, auto-detect interfaces.
3. [Control & introspection](python/control.md) — read and write features, walk the NodeMap, discover which features apply.
4. [Streaming](python/streaming.md) — context-manager streams, NumPy frames, pixel formats, timestamps.

## Reference

- [API reference](python/api.md) — every public class, function, and exception in one place.
- [Example scripts](https://github.com/VitalyVorobyev/viva-genicam/tree/main/crates/viva-pygenicam/examples) — runnable Python files mirroring the most common Rust examples.

## Supported

- Python 3.9+, abi3 wheels (one wheel covers every minor version).
- GigE Vision: discovery, control, streaming.
- USB3 Vision: discovery, control, streaming.
- Platforms with pre-built wheels: Linux x86_64 (manylinux_2_28), macOS arm64, Windows x86_64.

### Not exposed to Python yet

The Rust API is wider than the bindings. These exist in `viva-genicam` and have
no Python equivalent today:

- **Chunk data.** `frame.chunks` is not surfaced; `ChunkConfig` and
  `configure_chunks` are Rust-only.
- **Events.** There is no binding for the message channel or `EventStream`.
- **Time sync.** `time_calibrate` is not exposed, which also means
  `frame.ts_host` has no device mapping to work from — see
  [Streaming](python/streaming.md).
- **Action commands** and **FORCEIP / persistent IP** configuration.
- **Skipped nodes.** `NodeMap::skipped()` — the list of features we could not
  build from this camera's XML — is reachable from `viva-camctl` but not from
  Python (backlog DX-05). A missing feature is therefore indistinguishable from
  one the camera does not have.

If you need one of these, `viva-camctl` covers most of them from the command
line, and the [Rust crates](crates/README.md) cover all of them.

Need another platform? The sdist on PyPI builds from source — you'll need a Rust toolchain (`rustup`) and a C compiler. libusb is always statically vendored; no system package needed.
