# Camera Service API Specification

This document defines the **expected Rust library API** for the external camera service. The camera service lives in a **separate repository** and is not part of this workspace. This spec serves as:

1. A contract that the external service must satisfy
2. A blueprint for the mock camera service we build for testing (see Epic 1 in `docs/backlog.md`)
3. Documentation for how the Zenoh bridge maps library calls to Zenoh key expressions

The camera service uses GenTL internally to talk to physical cameras. A binary wrapper exposes the library API over Zenoh.

## Design Principles

1. **Generic and minimal** - The API surface is small. It exposes GenICam standard operations, not vendor-specific features.
2. **No Zenoh dependency** - The library crate has no networking code. Zenoh bridging is in the binary wrapper.
3. **Async-first** - All I/O operations are async (tokio).
4. **Error-rich** - All operations return `Result<T, CameraError>` with structured error types.

---

## Core Traits

### `CameraProvider`

Entry point for discovering and opening cameras.

```rust
#[async_trait]
pub trait CameraProvider: Send + Sync {
    /// Discover cameras visible through all loaded GenTL providers.
    /// Returns a snapshot of currently visible devices.
    async fn discover(&self) -> Result<Vec<CameraInfo>, CameraError>;

    /// Open a connection to a specific camera by its device ID.
    async fn open(&self, device_id: &str) -> Result<Box<dyn CameraHandle>, CameraError>;
}
```

### `CameraHandle`

Represents an open connection to a single camera.

```rust
#[async_trait]
pub trait CameraHandle: Send + Sync {
    /// Retrieve the full GenICam XML description.
    fn xml(&self) -> &str;

    /// Read the current value of a named node.
    async fn read_node(&self, name: &str) -> Result<NodeValue, CameraError>;

    /// Write a value to a named node.
    async fn write_node(&self, name: &str, value: &NodeValue) -> Result<(), CameraError>;

    /// Execute a Command node.
    async fn execute_command(&self, name: &str) -> Result<(), CameraError>;

    /// Subscribe to node value changes. Returns a receiver that yields
    /// (node_name, NodeValue) pairs as the camera reports them.
    fn subscribe_nodes(&self) -> Result<NodeValueReceiver, CameraError>;

    /// Start image acquisition.
    async fn start_acquisition(&self) -> Result<(), CameraError>;

    /// Stop image acquisition.
    async fn stop_acquisition(&self) -> Result<(), CameraError>;

    /// Subscribe to image frames. Returns a receiver that yields Frame objects.
    fn subscribe_frames(&self) -> Result<FrameReceiver, CameraError>;

    /// Get current acquisition statistics.
    fn acquisition_stats(&self) -> AcquisitionStats;

    /// Close the connection and release resources.
    async fn close(self: Box<Self>) -> Result<(), CameraError>;
}
```

---

## Data Types

### `CameraInfo`

```rust
pub struct CameraInfo {
    /// Unique device ID (from GenTL).
    pub device_id: String,
    /// Human-readable camera name.
    pub display_name: String,
    /// Camera model string.
    pub model: String,
    /// Serial number.
    pub serial: String,
    /// Transport layer type (GigE, USB3, CoaXPress, CameraLink).
    pub transport: TransportKind,
    /// GenTL provider that discovered this camera.
    pub provider_name: String,
}

pub enum TransportKind {
    GigEVision,
    Usb3Vision,
    CoaXPress,
    CameraLink,
    Custom(String),
}
```

### `NodeValue`

```rust
pub enum NodeValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    Enum(String),         // enum entry name
    Raw(Vec<u8>),         // register content
}
```

### `Frame`

```rust
pub struct Frame {
    /// Raw pixel data.
    pub data: Vec<u8>,
    /// Pixel format (SFNC name, e.g., "Mono8", "BayerRG8", "RGB8").
    pub pixel_format: PixelFormat,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Frame ID from the camera (monotonic).
    pub frame_id: u64,
    /// Timestamp from the camera (nanoseconds since epoch or device clock).
    pub timestamp_ns: u64,
}

pub enum PixelFormat {
    Mono8,
    Mono10,
    Mono12,
    Mono16,
    BayerRG8,
    BayerGR8,
    BayerBG8,
    BayerGB8,
    BayerRG10,
    BayerGR10,
    BayerBG10,
    BayerGB10,
    BayerRG12,
    BayerGR12,
    BayerBG12,
    BayerGB12,
    BayerRG16,
    BayerGR16,
    BayerBG16,
    BayerGB16,
    RGB8,
    BGR8,
    RGBa8,
    YCbCr422_8,
    YCbCr8,
    Coord3D_C16,
    Mono10p,
    Mono12p,
    Unknown(String),
}

pub struct AcquisitionStats {
    pub active: bool,
    pub fps: Option<f32>,
    pub frames_acquired: u64,
    pub frames_dropped: u64,
}
```

### `CameraError`

```rust
pub enum CameraError {
    /// No GenTL provider found on the system.
    NoProvider,
    /// GenTL provider failed to load.
    ProviderLoad { path: String, reason: String },
    /// Device not found during discovery.
    DeviceNotFound { device_id: String },
    /// Device is already open.
    AlreadyOpen { device_id: String },
    /// Node not found in the camera's feature tree.
    NodeNotFound { name: String },
    /// Node access denied (e.g., writing to a read-only node).
    AccessDenied { name: String, mode: String },
    /// Value out of range or invalid for the node.
    InvalidValue { name: String, reason: String },
    /// Acquisition error (start/stop/frame).
    Acquisition(String),
    /// Transport-level error.
    Transport(String),
    /// Timeout waiting for camera response.
    Timeout { operation: String },
    /// Generic error.
    Other(String),
}
```

---

## Channel Types

```rust
/// Receiver for node value updates.
pub type NodeValueReceiver = tokio::sync::mpsc::Receiver<(String, NodeValue)>;

/// Receiver for image frames.
pub type FrameReceiver = tokio::sync::mpsc::Receiver<Frame>;
```

---

## GenTL Provider Discovery

```rust
pub struct GenTlProviderManager {
    // ...
}

impl GenTlProviderManager {
    /// Scan standard paths for .cti files and load them.
    /// Paths checked (in order):
    /// 1. GENICAM_GENTL64_PATH / GENICAM_GENTL32_PATH environment variables
    /// 2. Platform-specific default locations
    /// 3. User-configured additional paths
    pub fn discover_providers() -> Result<Self, CameraError>;

    /// List loaded provider names.
    pub fn providers(&self) -> &[ProviderInfo];

    /// Create a CameraProvider that uses all loaded GenTL providers.
    pub fn create_provider(&self) -> Result<impl CameraProvider, CameraError>;
}

pub struct ProviderInfo {
    pub name: String,
    pub path: String,
    pub vendor: String,
    pub version: String,
}
```

---

## Zenoh Bridge (binary wrapper)

The binary wrapper maps the library API to Zenoh key expressions as defined in `docs/zenoh-api.md`:

| Library API | Zenoh Key | Direction |
|-------------|-----------|-----------|
| `discover()` | `genicam/devices/{id}/announce` | Service publishes periodically |
| `xml()` | `genicam/devices/{id}/xml` | Queryable (GET) |
| `read_node()` / `subscribe_nodes()` | `genicam/devices/{id}/nodes/{name}/value` | Service publishes on change |
| `write_node()` | `genicam/devices/{id}/nodes/{name}/set` | Queryable (GET with payload) |
| `execute_command()` | `genicam/devices/{id}/nodes/{name}/execute` | Queryable (GET) |
| `start_acquisition()` / `stop_acquisition()` | `genicam/devices/{id}/acquisition/control` | Queryable (GET with payload) |
| `acquisition_stats()` | `genicam/devices/{id}/acquisition/status` | Service publishes on change |
| `subscribe_frames()` | `genicam/devices/{id}/image` | Service publishes per frame |

---

## Zenoh API Extensions (updates to `viva-zenoh-api`)

### Image metadata key (new)

```
genicam/devices/{device_id}/image/meta
```

Published once at acquisition start and on format change:

```json
{
  "pixel_format": "Mono8",
  "width": 1920,
  "height": 1080,
  "payload_size": 2073600
}
```

This allows the streamer and any subscriber to configure their decoders without hardcoding dimensions.

### Node bulk read (new)

```
genicam/devices/{device_id}/nodes/bulk/read
```

Queryable that accepts a list of node names and returns all their current values in one round-trip:

```json
// Request
{ "names": ["ExposureTime", "Gain", "Width", "Height"] }

// Response
{
  "values": {
    "ExposureTime": { "value": 10000.0, "access_mode": "RW" },
    "Gain": { "value": 1.5, "access_mode": "RW" },
    "Width": { "value": 1920, "access_mode": "RW" },
    "Height": { "value": 1080, "access_mode": "RW" }
  }
}
```
