//! End-to-end integration tests for the GenICam mock service.
//!
//! Each test spins up the relevant mock service task(s) in-process using
//! a real Zenoh peer-to-peer session pair (no external router needed).
//! Unique `device_id` values prevent cross-test interference when tests
//! run in parallel.

use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;

use viva_mock_service::{config::MockConfig, state::NodeStore};
use viva_zenoh_api::{
    BulkReadRequest, BulkReadResponse, DeviceAnnounce, DeviceXmlResponse, NodeOpResponse,
    NodeSetRequest,
};

const TEST_XML: &str = include_str!("../fixtures/sfnc-standard.xml");

fn make_config(device_id: &str) -> MockConfig {
    MockConfig::new(
        device_id,
        "CI Test Camera",
        "TestModel-CI",
        "CI-SN-001",
        320,
        240,
        10.0,
    )
}

async fn open_zenoh() -> Arc<zenoh::Session> {
    Arc::new(
        zenoh::open(zenoh::Config::default())
            .await
            .expect("open zenoh session"),
    )
}

/// Discovery: the mock service must announce itself with the correct fields
/// and the current `API_VERSION`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_discovery_announcement() {
    let config = make_config("ci01-discovery");
    let svc = open_zenoh().await;
    let client = open_zenoh().await;

    let sub = client
        .declare_subscriber(&viva_zenoh_api::keys::announce(&config.device_id))
        .await
        .expect("declare subscriber");

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(viva_mock_service::discovery::run(
        svc.clone(),
        config.clone(),
        shutdown_rx,
    ));

    let sample = timeout(Duration::from_secs(10), sub.recv_async())
        .await
        .expect("timeout waiting for announce")
        .expect("recv announce");

    let announce: DeviceAnnounce =
        serde_json::from_slice(&sample.payload().to_bytes()).expect("parse DeviceAnnounce");

    assert_eq!(announce.id, config.device_id);
    assert_eq!(announce.name, config.device_name);
    assert_eq!(announce.model, config.model);
    assert_eq!(announce.serial, config.serial);
    assert_eq!(
        announce.api_version,
        Some(viva_zenoh_api::API_VERSION),
        "api_version must equal API_VERSION={}",
        viva_zenoh_api::API_VERSION
    );

    let _ = shutdown_tx.send(true);
}

/// XML queryable: the mock service must return parseable GenICam XML when queried.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_xml_queryable() {
    let config = make_config("ci01-xml");
    let svc = open_zenoh().await;
    let client = open_zenoh().await;

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(viva_mock_service::xml_queryable::run(
        svc.clone(),
        config.clone(),
        TEST_XML.to_string(),
        shutdown_rx,
    ));

    // Allow the queryable to register before issuing the GET.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let key = viva_zenoh_api::keys::xml(&config.device_id);
    let replies = client.get(&key).await.expect("get XML");

    let reply = timeout(Duration::from_secs(10), replies.recv_async())
        .await
        .expect("timeout waiting for XML reply")
        .expect("recv reply");

    let sample = reply.result().expect("XML reply must be ok");
    let resp: DeviceXmlResponse =
        serde_json::from_slice(&sample.payload().to_bytes()).expect("parse DeviceXmlResponse");

    assert!(
        resp.xml.contains("<RegisterDescription"),
        "XML response must contain <RegisterDescription; got prefix: {}",
        &resp.xml[..resp.xml.len().min(200)]
    );

    let _ = shutdown_tx.send(true);
}

/// Bulk read: the mock service returns Width and Height matching the config.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_bulk_read_queryable() {
    let config = make_config("ci01-bulk");
    let svc = open_zenoh().await;
    let client = open_zenoh().await;

    let graph = viva_xml_model::parse_genicam_xml(TEST_XML).expect("parse XML");
    let store = Arc::new(NodeStore::from_graph(&graph, &config));

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(viva_mock_service::nodes::run_bulk_read_queryable(
        svc.clone(),
        config.clone(),
        store.clone(),
        shutdown_rx,
    ));

    tokio::time::sleep(Duration::from_millis(200)).await;

    let key = viva_zenoh_api::keys::nodes_bulk_read(&config.device_id);
    let req = BulkReadRequest {
        names: vec!["Width".to_string(), "Height".to_string()],
    };
    let payload = serde_json::to_vec(&req).expect("serialize BulkReadRequest");
    let replies = client
        .get(&key)
        .payload(payload)
        .await
        .expect("get bulk read");

    let reply = timeout(Duration::from_secs(10), replies.recv_async())
        .await
        .expect("timeout waiting for bulk read reply")
        .expect("recv reply");

    let sample = reply.result().expect("bulk read reply must be ok");
    let resp: BulkReadResponse =
        serde_json::from_slice(&sample.payload().to_bytes()).expect("parse BulkReadResponse");

    assert!(
        resp.values.contains_key("Width"),
        "Width must be in response"
    );
    assert!(
        resp.values.contains_key("Height"),
        "Height must be in response"
    );

    let width = resp.values["Width"]
        .value
        .as_u64()
        .expect("Width is a number");
    let height = resp.values["Height"]
        .value
        .as_u64()
        .expect("Height is a number");
    assert_eq!(width, 320, "Width should match config.width");
    assert_eq!(height, 240, "Height should match config.height");

    let _ = shutdown_tx.send(true);
}

/// Node set: writing a new Width value must return ok=true and update the store.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_node_set_queryable() {
    let config = make_config("ci01-set");
    let svc = open_zenoh().await;
    let client = open_zenoh().await;

    let graph = viva_xml_model::parse_genicam_xml(TEST_XML).expect("parse XML");
    let store = Arc::new(NodeStore::from_graph(&graph, &config));

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(viva_mock_service::nodes::run_set_queryable(
        svc.clone(),
        config.clone(),
        store.clone(),
        shutdown_rx,
    ));

    tokio::time::sleep(Duration::from_millis(200)).await;

    let key = viva_zenoh_api::keys::node_set(&config.device_id, "Width");
    let req = NodeSetRequest {
        value: serde_json::json!(512),
    };
    let payload = serde_json::to_vec(&req).expect("serialize NodeSetRequest");
    let replies = client
        .get(&key)
        .payload(payload)
        .await
        .expect("get node set");

    let reply = timeout(Duration::from_secs(10), replies.recv_async())
        .await
        .expect("timeout waiting for set reply")
        .expect("recv reply");

    let sample = reply.result().expect("set reply must be ok");
    let resp: NodeOpResponse =
        serde_json::from_slice(&sample.payload().to_bytes()).expect("parse NodeOpResponse");

    assert!(
        resp.ok,
        "set Width=512 should succeed; error: {:?}",
        resp.error
    );

    // Verify the value was actually persisted in the store.
    let stored = store
        .get_value("Width")
        .await
        .expect("Width must exist in store after set");
    assert_eq!(stored.as_u64(), Some(512), "stored Width should be 512");

    let _ = shutdown_tx.send(true);
}
