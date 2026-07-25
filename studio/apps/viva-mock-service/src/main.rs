use std::sync::Arc;

use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

use viva_mock_service::{
    acquisition, config::MockConfig, discovery, nodes, state::NodeStore, status, xml_queryable,
};

#[derive(Parser, Debug)]
#[command(
    name = "viva-mock-service",
    about = "Mock GenICam camera service over Zenoh"
)]
pub struct Cli {
    /// Device ID
    #[arg(long, default_value = "mock-cam0")]
    device_id: String,

    /// Display name
    #[arg(long, default_value = "Mock Camera 1")]
    device_name: String,

    /// Model string
    #[arg(long, default_value = "MockCam-1000")]
    model: String,

    /// Serial number
    #[arg(long, default_value = "MOCK00001")]
    serial: String,

    /// Image width in pixels
    #[arg(long, default_value_t = 640)]
    width: u32,

    /// Image height in pixels
    #[arg(long, default_value_t = 480)]
    height: u32,

    /// Acquisition frame rate
    #[arg(long, default_value_t = 30.0)]
    fps: f32,

    /// GenICam XML fixture to serve (defaults to built-in SFNC fixture)
    #[arg(long)]
    fixture: Option<String>,

    /// Zenoh configuration file
    #[arg(long)]
    zenoh_config: Option<String>,
}

const DEFAULT_FIXTURE: &str = include_str!("../fixtures/sfnc-standard.xml");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    init_tracing();
    let cli = Cli::parse();
    let config = MockConfig::new(
        &cli.device_id,
        &cli.device_name,
        &cli.model,
        &cli.serial,
        cli.width,
        cli.height,
        cli.fps,
    );

    let zenoh_config = load_zenoh_config(cli.zenoh_config.as_deref())?;
    let session = Arc::new(zenoh::open(zenoh_config).await?);

    let xml = load_fixture_xml(cli.fixture.as_deref())?;
    let graph = viva_xml_model::parse_genicam_xml(&xml)?;

    let node_store = Arc::new(NodeStore::from_graph(&graph, &config));

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    info!(
        "Starting mock camera service: id={}, name={}, {}x{} @ {} fps",
        config.device_id, config.device_name, config.width, config.height, config.fps
    );

    let tasks = vec![
        tokio::spawn(discovery::run(
            session.clone(),
            config.clone(),
            shutdown_rx.clone(),
        )),
        tokio::spawn(xml_queryable::run(
            session.clone(),
            config.clone(),
            xml.clone(),
            shutdown_rx.clone(),
        )),
        tokio::spawn(nodes::run_publisher(
            session.clone(),
            config.clone(),
            node_store.clone(),
            shutdown_rx.clone(),
        )),
        tokio::spawn(nodes::run_set_queryable(
            session.clone(),
            config.clone(),
            node_store.clone(),
            shutdown_rx.clone(),
        )),
        tokio::spawn(nodes::run_execute_queryable(
            session.clone(),
            config.clone(),
            shutdown_rx.clone(),
        )),
        tokio::spawn(nodes::run_bulk_read_queryable(
            session.clone(),
            config.clone(),
            node_store.clone(),
            shutdown_rx.clone(),
        )),
        tokio::spawn(acquisition::run(
            session.clone(),
            config.clone(),
            node_store.clone(),
            shutdown_rx.clone(),
        )),
        tokio::spawn(status::run(
            session.clone(),
            config.clone(),
            shutdown_rx.clone(),
        )),
    ];

    tokio::signal::ctrl_c().await?;
    info!("Shutdown requested (CTRL+C)");
    let _ = shutdown_tx.send(true);

    for task in tasks {
        let _ = task.await;
    }

    session.close().await?;
    info!("Mock camera service shut down");
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn load_fixture_xml(
    path: Option<&str>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    match path {
        Some(p) => Ok(std::fs::read_to_string(p)?),
        None => Ok(DEFAULT_FIXTURE.to_string()),
    }
}

fn load_zenoh_config(
    input: Option<&str>,
) -> Result<zenoh::Config, Box<dyn std::error::Error + Send + Sync>> {
    match input {
        None => Ok(zenoh::Config::default()),
        Some(value) => {
            let path = std::path::Path::new(value);
            if path.exists() {
                Ok(zenoh::Config::from_file(path)?)
            } else {
                Ok(zenoh::Config::from_json5(value)?)
            }
        }
    }
}
