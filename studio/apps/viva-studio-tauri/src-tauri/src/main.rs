#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backend;
mod commands;
mod error;
mod state;

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::prelude::*;

use backend::BackendState;

#[tauri::command]
fn ping() -> &'static str {
    "pong"
}

/// Determine whether the app should run in remote (Zenoh) mode.
///
/// Returns `Some(config)` only if the `ZENOH_CONFIG` environment variable is set.
/// Embedded mode (direct camera access) is the default for all other cases.
fn detect_zenoh_config() -> Option<zenoh::Config> {
    if let Ok(path) = std::env::var("ZENOH_CONFIG") {
        match zenoh::Config::from_file(&path) {
            Ok(cfg) => {
                tracing::info!("Loaded Zenoh config from ZENOH_CONFIG={path} — remote mode");
                return Some(cfg);
            }
            Err(e) => tracing::warn!("Failed to load ZENOH_CONFIG={path}: {e}"),
        }
    }

    tracing::info!("No ZENOH_CONFIG set — using embedded mode (direct camera access)");
    None
}

fn dirs_log_path() -> std::path::PathBuf {
    let base = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let log_dir = base.join(".viva-studio").join("logs");
    std::fs::create_dir_all(&log_dir).ok();
    log_dir
}

fn main() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "viva_studio=info,warn".into());

    // File appender: daily rotation in ~/.viva-studio/logs/
    let log_dir = dirs_log_path();
    let file_appender = tracing_appender::rolling::daily(&log_dir, "studio.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(non_blocking),
        )
        .init();

    tracing::info!("Log directory: {}", log_dir.display());

    // Detect backend mode: embedded (default) or remote (Zenoh).
    let zenoh_config = detect_zenoh_config();
    let is_remote_mode = zenoh_config.is_some();

    // ModelState: holds the last parsed UiGraph (used by xml_model commands).
    let model_state = RwLock::new(state::ModelState::default());

    // ZenohState: device discovery, connection, node cache, acquisition.
    // Wrapped in Arc so background tasks can hold a clone independently.
    let zenoh_state = Arc::new(state::ZenohState::new());
    let zenoh_for_setup = zenoh_state.clone();

    // SfncGroupsState: cached SFNC group config loaded once from the bundled JSON.
    let sfnc_groups_state: commands::sfnc_groups::SfncGroupsState =
        Arc::new(RwLock::new(None::<Vec<commands::sfnc_groups::SfncGroup>>));

    // Create the appropriate backend.
    let embedded_backend: Option<Arc<backend::embedded::EmbeddedBackend>> = if is_remote_mode {
        tracing::info!("Starting in REMOTE mode (Zenoh service)");
        None
    } else {
        tracing::info!("Starting in EMBEDDED mode (direct camera access)");
        Some(Arc::new(backend::embedded::EmbeddedBackend::new()))
    };

    let backend_state: BackendState = match &embedded_backend {
        Some(eb) => eb.clone(),
        None => Arc::new(backend::remote::RemoteBackend),
    };

    // Clone for the setup closure.
    let embedded_for_setup = embedded_backend.clone();

    if let Err(err) = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(model_state)
        .manage(zenoh_state)
        .manage(sfnc_groups_state)
        .manage(backend_state)
        .setup(move |app| {
            let app_handle = app.handle().clone();

            if is_remote_mode {
                // Remote mode: initialize Zenoh and start discovery via Zenoh subscribers.
                let zenoh = zenoh_for_setup.clone();
                let zenoh_config = zenoh_config.expect("zenoh_config must be Some in remote mode");
                tauri::async_runtime::spawn(async move {
                    match zenoh::open(zenoh_config).await {
                        Ok(session) => {
                            tracing::info!("Zenoh session open, ZID: {}", session.zid());
                            *zenoh.session.lock().await = Some(Arc::new(session));
                            commands::device::start_discovery_task(zenoh, app_handle);
                        }
                        Err(e) => {
                            tracing::error!("Failed to open Zenoh session: {e}");
                        }
                    }
                });
            } else if let Some(embedded) = embedded_for_setup {
                // Embedded mode: start direct device discovery.
                embedded.start_discovery_task(app_handle, std::time::Duration::from_secs(3));
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            // Backend mode query
            commands::backend::backend_mode,
            // XML model (offline + fixture mode)
            commands::xml_model::parse_xml,
            commands::xml_model::get_current_model,
            commands::xml_model::list_fixtures,
            commands::xml_model::load_fixture,
            // Device discovery & connection (Zenoh-based, backward compatible)
            commands::device::list_discovered_devices,
            commands::device::get_connection_state,
            commands::device::connect_device,
            commands::device::disconnect_device,
            // Node operations (Zenoh-based, backward compatible)
            commands::nodes::get_node_value,
            commands::nodes::write_node,
            commands::nodes::execute_command,
            commands::nodes::read_nodes_bulk,
            // Live feature state (FeatureState contract — authoritative for the UI)
            commands::nodes::query_feature_state,
            commands::nodes::query_feature_states_bulk,
            // Acquisition (Zenoh-based, backward compatible)
            commands::acquisition::get_acquisition_status,
            commands::acquisition::start_acquisition,
            commands::acquisition::stop_acquisition,
            // Recording
            commands::recording::start_recording,
            commands::recording::stop_recording,
            commands::recording::get_recording_status,
            // SFNC groups config
            commands::sfnc_groups::get_sfnc_groups,
            // Embedded backend commands
            commands::embedded_device::embedded_discover,
            commands::embedded_device::embedded_connect,
            commands::embedded_device::embedded_disconnect,
            commands::embedded_device::embedded_get_feature,
            commands::embedded_device::embedded_set_feature,
            commands::embedded_device::embedded_exec_command,
            commands::embedded_device::embedded_bulk_read,
            commands::embedded_device::embedded_start_acquisition,
            commands::embedded_device::embedded_stop_acquisition,
            // IP configuration (embedded mode only)
            commands::ip_config::force_ip,
            commands::ip_config::get_network_config,
            commands::ip_config::set_persistent_ip,
        ])
        .run(tauri::generate_context!())
    {
        tracing::error!("error while running tauri application: {err}");
    }
}
