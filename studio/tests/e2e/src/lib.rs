//! E2E test harness for GenICam Studio.
//!
//! Spawns `arv-fake-gv-camera-0.8` and `genicam-service` as child processes,
//! waits for device discovery, and provides a Zenoh session for test assertions.
//!
//! All tests are `#[ignore]` — they require external binaries and are run via:
//! ```bash
//! cargo test -p e2e-tests -- --ignored --test-threads=1
//! ```

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use tokio::process::{Child, Command};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

use viva_zenoh_api::DeviceAnnounce;

/// Environment variable for the genicam-service binary path.
const SERVICE_PATH_ENV: &str = "GENICAM_SERVICE_PATH";
/// Preferred default service binary path relative to the shared `vision/` parent.
const SERVICE_PATH_PRIMARY_DEFAULT: &str =
    "genicam-rs/crates/genicam-service/target/debug/genicam-service";
/// Legacy fallback service binary path relative to the shared `vision/` parent.
const SERVICE_PATH_FALLBACK_DEFAULT: &str = "genicam-rs/target/debug/genicam-service";

/// Environment variable for the fake camera binary path.
const FAKE_CAM_PATH_ENV: &str = "ARV_FAKE_CAMERA_PATH";
/// Default: find in PATH.
const FAKE_CAM_PATH_DEFAULT: &str = "arv-fake-gv-camera-0.8";

/// Environment variable for the service-side Zenoh config file (JSON5).
const SERVICE_ZENOH_CONFIG_ENV: &str = "GENICAM_SERVICE_ZENOH_CONFIG";
/// Environment variable for the client/test-side Zenoh config file (JSON5).
const CLIENT_ZENOH_CONFIG_ENV: &str = "GENICAM_CLIENT_ZENOH_CONFIG";
/// Legacy environment variable: applies the same Zenoh config to service + client.
const LEGACY_ZENOH_CONFIG_ENV: &str = "ZENOH_CONFIG";

/// Default service-side Zenoh config used by the manual macOS topology.
const SERVICE_ZENOH_CONFIG_DEFAULT: &str = "config/zenoh-local.json5";
/// Default client/test-side Zenoh config used by the manual macOS topology.
const CLIENT_ZENOH_CONFIG_DEFAULT: &str = "config/zenoh-studio.json5";

/// The device ID produced by the aravis fake camera (all-zero MAC).
pub const FAKE_DEVICE_ID: &str = "cam-000000000000";

/// How long to wait for the first announce message from genicam-service.
const ANNOUNCE_TIMEOUT: Duration = Duration::from_secs(15);

/// Test harness that manages the fake camera and service lifecycle.
pub struct TestHarness {
    fake_camera: Child,
    service: Child,
    session: Arc<zenoh::Session>,
    device_id: String,
}

#[derive(Debug)]
pub enum HarnessError {
    FakeCameraSpawn(String),
    ServiceSpawn(String),
    ZenohOpen(String),
    Timeout(String),
}

impl std::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FakeCameraSpawn(e) => write!(f, "Failed to spawn fake camera: {e}"),
            Self::ServiceSpawn(e) => write!(f, "Failed to spawn service: {e}"),
            Self::ZenohOpen(e) => write!(f, "Failed to open Zenoh session: {e}"),
            Self::Timeout(e) => write!(f, "Timeout: {e}"),
        }
    }
}

impl std::error::Error for HarnessError {}

#[derive(Debug, Clone)]
pub struct HarnessOptions {
    pub fake_cam_path: String,
    pub service_path: String,
    pub service_zenoh_config: Option<String>,
    pub client_zenoh_config: Option<String>,
}

impl Default for HarnessOptions {
    fn default() -> Self {
        Self::from_env()
    }
}

impl HarnessOptions {
    pub fn from_env() -> Self {
        let legacy_zenoh = std::env::var(LEGACY_ZENOH_CONFIG_ENV).ok();

        Self {
            fake_cam_path: std::env::var(FAKE_CAM_PATH_ENV)
                .unwrap_or_else(|_| FAKE_CAM_PATH_DEFAULT.to_string()),
            service_path: std::env::var(SERVICE_PATH_ENV)
                .unwrap_or_else(|_| resolve_default_service_path()),
            service_zenoh_config: std::env::var(SERVICE_ZENOH_CONFIG_ENV)
                .ok()
                .or_else(|| legacy_zenoh.clone())
                .or_else(|| existing_workspace_path(SERVICE_ZENOH_CONFIG_DEFAULT)),
            client_zenoh_config: std::env::var(CLIENT_ZENOH_CONFIG_ENV)
                .ok()
                .or(legacy_zenoh)
                .or_else(|| existing_workspace_path(CLIENT_ZENOH_CONFIG_DEFAULT)),
        }
    }
}

impl TestHarness {
    /// Start the fake camera, genicam-service, and a Zenoh test session.
    ///
    /// Waits for the service to publish its first announce message before returning.
    pub async fn start() -> Result<Self, HarnessError> {
        Self::start_with_options(HarnessOptions::from_env()).await
    }

    /// Start the harness with explicit process paths and Zenoh configs.
    pub async fn start_with_options(options: HarnessOptions) -> Result<Self, HarnessError> {
        init_tracing();

        let HarnessOptions {
            fake_cam_path,
            service_path,
            service_zenoh_config,
            client_zenoh_config,
        } = options;

        // 1. Start the fake GigE camera on loopback
        tracing::info!("Starting fake camera: {fake_cam_path}");
        let fake_camera = Command::new(&fake_cam_path)
            .arg("-i")
            .arg("127.0.0.1")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                HarnessError::FakeCameraSpawn(format!(
                    "{e} (path: {fake_cam_path}). Is aravis installed?"
                ))
            })?;

        // 2. Wait for the camera to be ready (brief delay for UDP socket binding)
        sleep(Duration::from_secs(2)).await;

        // 3. Start genicam-service with loopback interface
        tracing::info!("Starting service: {service_path}");
        let mut svc_cmd = Command::new(&service_path);
        svc_cmd
            .arg("--iface")
            .arg(if cfg!(target_os = "macos") {
                "lo0"
            } else {
                "lo"
            })
            .stdout(Stdio::null())
            // Inherit stderr so manual/ignored e2e runs show service-side
            // acquisition diagnostics such as the first GVSP/image frame.
            .stderr(Stdio::inherit());

        // Pass service-side Zenoh config if available.
        if let Some(ref cfg_path) = service_zenoh_config {
            tracing::info!("Service Zenoh config: {cfg_path}");
            svc_cmd.arg("--zenoh-config").arg(cfg_path);
        }

        let service = svc_cmd.spawn().map_err(|e| {
            HarnessError::ServiceSpawn(format!(
                "{e} (path: {service_path}). Build it with: cd ../genicam-rs/crates/genicam-service && cargo build"
            ))
        })?;

        // 4. Open the client-side Zenoh session.
        let zenoh_config = match &client_zenoh_config {
            Some(path) => zenoh::Config::from_file(path)
                .map_err(|e| HarnessError::ZenohOpen(format!("config {path}: {e}")))?,
            None => zenoh::Config::default(),
        };
        if let Some(ref cfg_path) = client_zenoh_config {
            tracing::info!("Client Zenoh config: {cfg_path}");
        }
        let session = Arc::new(
            zenoh::open(zenoh_config)
                .await
                .map_err(|e| HarnessError::ZenohOpen(e.to_string()))?,
        );

        // 5. Wait for the first announce message
        tracing::info!("Waiting for device announce...");
        let sub = session
            .declare_subscriber(viva_zenoh_api::keys::ANNOUNCE_ALL)
            .await
            .map_err(|e| HarnessError::ZenohOpen(format!("subscriber: {e}")))?;

        let announce = timeout(ANNOUNCE_TIMEOUT, async {
            loop {
                if let Ok(sample) = sub.recv_async().await {
                    let bytes = sample.payload().to_bytes();
                    if let Ok(a) = serde_json::from_slice::<DeviceAnnounce>(&bytes) {
                        return a;
                    }
                }
            }
        })
        .await
        .map_err(|_| {
            HarnessError::Timeout(format!(
                "No announce received within {}s. Is genicam-service running?",
                ANNOUNCE_TIMEOUT.as_secs()
            ))
        })?;

        let device_id = announce.id.clone();
        tracing::info!("Device discovered: {device_id} ({})", announce.name);

        // Give Zenoh time to fully establish bidirectional transport
        // and propagate queryable declarations across peers.
        // pub/sub works quickly but queryables with wildcards need more time.
        sleep(Duration::from_secs(5)).await;

        Ok(Self {
            fake_camera,
            service,
            session,
            device_id,
        })
    }

    /// The device ID of the discovered fake camera.
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// The Zenoh session for test assertions.
    pub fn session(&self) -> &Arc<zenoh::Session> {
        &self.session
    }

    /// Kill the fake camera process (for device-lost tests).
    pub async fn kill_fake_camera(&mut self) -> Result<(), std::io::Error> {
        self.fake_camera.kill().await
    }

    /// Graceful shutdown: kill service first, then fake camera.
    pub async fn shutdown(&mut self) {
        let _ = self.service.kill().await;
        let _ = self.fake_camera.kill().await;
    }
}

pub struct EmbeddedStreamerHarness {
    pub ws_url: String,
    shutdown_tx: watch::Sender<bool>,
    task_handles: Vec<JoinHandle<()>>,
}

impl EmbeddedStreamerHarness {
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        for handle in self.task_handles {
            handle.abort();
            let _ = handle.await;
        }
    }
}

impl Drop for TestHarness {
    fn drop(&mut self) {
        // Best-effort synchronous kill (async Drop not available)
        let _ = self.service.start_kill();
        let _ = self.fake_camera.start_kill();
    }
}

pub fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

pub fn json_value_as_u32(v: &serde_json::Value) -> Option<u32> {
    v.as_u64()
        .map(|n| n as u32)
        .or_else(|| v.as_f64().map(|f| f as u32))
        .or_else(|| v.as_str().and_then(|s| s.parse::<u32>().ok()))
}

pub async fn start_embedded_streamer(
    session: Arc<zenoh::Session>,
    image_key: String,
    width: u32,
    height: u32,
) -> Result<EmbeddedStreamerHarness, String> {
    let meta_key = viva_streamer::meta::derive_meta_key(&image_key);

    let (frame_tx, _) = watch::channel(Bytes::new());
    let info_tx = watch::channel(viva_streamer::ws::StreamInfo::from_image_meta(
        &viva_streamer::meta::default_image_meta(width, height),
    ))
    .0;
    let shared_meta = Arc::new(tokio::sync::RwLock::new(
        viva_streamer::meta::default_image_meta(width, height),
    ));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("bind WS listener: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("get WS local address: {e}"))?
        .port();
    let ws_url = format!("ws://127.0.0.1:{port}/ws");

    tracing::info!("Embedded streamer test WS on {ws_url}");

    let source_config = viva_streamer::zenoh_source::ZenohSourceConfig {
        key_expr: image_key,
        meta_key,
        fps_limit: Some(30),
    };

    let zenoh_handle = tokio::spawn({
        let session = session.clone();
        let shared_meta = shared_meta.clone();
        let frame_tx = frame_tx.clone();
        let info_tx = info_tx.clone();
        let shutdown_rx = shutdown_rx.clone();
        async move {
            if let Err(err) = viva_streamer::zenoh_source::run_with_session(
                session,
                source_config,
                shared_meta,
                frame_tx,
                info_tx,
                shutdown_rx,
            )
            .await
            {
                tracing::error!("Embedded streamer Zenoh source failed: {err}");
            }
        }
    });

    let ws_handle = tokio::spawn({
        let state = viva_streamer::ws::AppState { frame_tx, info_tx };
        let shutdown_rx = shutdown_rx.clone();
        async move {
            if let Err(err) = viva_streamer::ws::run_server_with_listener(
                listener,
                "/ws".to_string(),
                state,
                shutdown_rx,
            )
            .await
            {
                tracing::error!("Embedded streamer WS server failed: {err}");
            }
        }
    });

    Ok(EmbeddedStreamerHarness {
        ws_url,
        shutdown_tx,
        task_handles: vec![zenoh_handle, ws_handle],
    })
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "e2e_tests=info,warn".into()),
        )
        .try_init();
}

fn resolve_default_service_path() -> String {
    for candidate in [SERVICE_PATH_PRIMARY_DEFAULT, SERVICE_PATH_FALLBACK_DEFAULT] {
        let path = sibling_path(candidate);
        if path.exists() {
            let path = path.canonicalize().unwrap_or(path);
            return path.to_string_lossy().into_owned();
        }
    }
    sibling_path(SERVICE_PATH_PRIMARY_DEFAULT)
        .to_string_lossy()
        .into_owned()
}

fn existing_workspace_path(relative: &str) -> Option<String> {
    let path = workspace_path(relative);
    if !path.exists() {
        return None;
    }
    let path = path.canonicalize().unwrap_or(path);
    Some(path.to_string_lossy().into_owned())
}

fn sibling_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

// ── Helpers for tests ────────────────────────────────────────────────────────

/// Fetch the GenICam XML for a device via Zenoh GET.
pub async fn fetch_xml(session: &zenoh::Session, device_id: &str) -> Result<String, String> {
    let key = viva_zenoh_api::keys::xml(device_id);
    let replies = session
        .get(&key)
        .timeout(Duration::from_secs(10))
        .await
        .map_err(|e| format!("GET error: {e}"))?;

    match replies.recv_async().await {
        Ok(reply) => match reply.result() {
            Ok(sample) => {
                let bytes = sample.payload().to_bytes();
                let resp: viva_zenoh_api::DeviceXmlResponse =
                    serde_json::from_slice(&bytes).map_err(|e| format!("parse: {e}"))?;
                Ok(resp.xml)
            }
            Err(e) => Err(format!("reply error: {e}")),
        },
        Err(_) => Err("no reply (timeout)".to_string()),
    }
}

/// Write a node value via Zenoh GET with payload.
pub async fn write_node(
    session: &zenoh::Session,
    device_id: &str,
    node_name: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    let key = viva_zenoh_api::keys::node_set(device_id, node_name);
    let payload =
        serde_json::to_vec(&viva_zenoh_api::NodeSetRequest { value }).map_err(|e| e.to_string())?;

    let replies = session
        .get(&key)
        .payload(payload)
        .timeout(Duration::from_secs(5))
        .await
        .map_err(|e| format!("GET error: {e}"))?;

    match replies.recv_async().await {
        Ok(reply) => match reply.result() {
            Ok(sample) => {
                let bytes = sample.payload().to_bytes();
                let resp: viva_zenoh_api::NodeOpResponse =
                    serde_json::from_slice(&bytes).map_err(|e| format!("parse: {e}"))?;
                if resp.ok {
                    Ok(())
                } else {
                    Err(resp.error.unwrap_or_else(|| "write failed".to_string()))
                }
            }
            Err(e) => Err(format!("reply error: {e}")),
        },
        Err(_) => Err("no reply (timeout)".to_string()),
    }
}

/// Query the full `FeatureState` for a single node via the
/// `nodes/{name}/state` queryable (API v2).
pub async fn query_feature_state(
    session: &zenoh::Session,
    device_id: &str,
    node_name: &str,
) -> Result<viva_zenoh_api::FeatureState, String> {
    let key = viva_zenoh_api::keys::node_introspect(device_id, node_name);
    let replies = session
        .get(&key)
        .timeout(Duration::from_secs(5))
        .await
        .map_err(|e| format!("GET error: {e}"))?;

    match replies.recv_async().await {
        Ok(reply) => match reply.result() {
            Ok(sample) => {
                let bytes = sample.payload().to_bytes();
                serde_json::from_slice(&bytes).map_err(|e| format!("parse: {e}"))
            }
            Err(e) => Err(format!("reply error: {e}")),
        },
        Err(_) => Err("no reply (timeout)".to_string()),
    }
}

/// Bulk-read node values via Zenoh GET with payload.
pub async fn read_bulk(
    session: &zenoh::Session,
    device_id: &str,
    names: &[&str],
) -> Result<viva_zenoh_api::BulkReadResponse, String> {
    let key = viva_zenoh_api::keys::nodes_bulk_read(device_id);
    let payload = serde_json::to_vec(&viva_zenoh_api::BulkReadRequest {
        names: names.iter().map(|s| s.to_string()).collect(),
    })
    .map_err(|e| e.to_string())?;

    let replies = session
        .get(&key)
        .payload(payload)
        .timeout(Duration::from_secs(5))
        .await
        .map_err(|e| format!("GET error: {e}"))?;

    match replies.recv_async().await {
        Ok(reply) => match reply.result() {
            Ok(sample) => {
                let bytes = sample.payload().to_bytes();
                serde_json::from_slice(&bytes).map_err(|e| format!("parse: {e}"))
            }
            Err(e) => Err(format!("reply error: {e}")),
        },
        Err(_) => Err("no reply (timeout)".to_string()),
    }
}

/// Send an acquisition command (Start/Stop).
pub async fn send_acquisition_command(
    session: &zenoh::Session,
    device_id: &str,
    command: viva_zenoh_api::AcquisitionCommand,
) -> Result<(), String> {
    let key = viva_zenoh_api::keys::acquisition_control(device_id);
    let payload = serde_json::to_vec(&viva_zenoh_api::AcquisitionControlRequest { command })
        .map_err(|e| e.to_string())?;

    let replies = session
        .get(&key)
        .payload(payload)
        .timeout(Duration::from_secs(5))
        .await
        .map_err(|e| format!("GET error: {e}"))?;

    match replies.recv_async().await {
        Ok(reply) => match reply.result() {
            Ok(sample) => {
                let bytes = sample.payload().to_bytes();
                let resp: viva_zenoh_api::NodeOpResponse =
                    serde_json::from_slice(&bytes).map_err(|e| format!("parse: {e}"))?;
                if resp.ok {
                    Ok(())
                } else {
                    Err(resp.error.unwrap_or_else(|| "command failed".to_string()))
                }
            }
            Err(e) => Err(format!("reply error: {e}")),
        },
        Err(_) => Err("no reply (timeout)".to_string()),
    }
}
