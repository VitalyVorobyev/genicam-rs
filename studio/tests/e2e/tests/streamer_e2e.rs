//! Streamer integration test: synthetic Zenoh frame -> BMP over WebSocket.
//!
//! This test exercises the exact code path the Tauri app uses:
//! Zenoh subscriber -> FrameHeader decode -> BMP encode -> WS broadcast.
//!
//! No external binaries required. Run with:
//! ```sh
//! cargo test -p e2e-tests --test streamer_e2e
//! ```

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use tokio::sync::watch;
use viva_zenoh_api::{FrameHeader, ImageMeta, PixelFormat};

/// Publish a synthetic Mono8 frame on Zenoh, run the embedded streamer,
/// connect as a WS client, and verify a valid BMP frame arrives.
#[tokio::test(flavor = "multi_thread")]
async fn test_streamer_synthetic_frame() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("e2e_tests=info,viva_streamer=debug,warn")
        .try_init();

    let width: u32 = 64;
    let height: u32 = 48;
    let device_id = "test-synthetic";

    // 1. Open Zenoh session (in-memory, no network).
    let session = Arc::new(zenoh::open(zenoh::Config::default()).await.unwrap());

    let image_key = viva_zenoh_api::keys::image(device_id);
    let meta_key = viva_streamer::meta::derive_meta_key(&image_key);

    // 2. Set up the embedded streamer (same code as Tauri's start_acquisition).
    let (frame_tx, _) = watch::channel(Bytes::new());
    let info_tx = watch::channel(viva_streamer::ws::StreamInfo::from_image_meta(
        &viva_streamer::meta::default_image_meta(width, height),
    ))
    .0;
    let shared_meta = Arc::new(tokio::sync::RwLock::new(
        viva_streamer::meta::default_image_meta(width, height),
    ));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Bind WS server.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let ws_url = format!("ws://127.0.0.1:{port}/ws");

    // Spawn Zenoh source task.
    let source_config = viva_streamer::zenoh_source::ZenohSourceConfig {
        key_expr: image_key.clone(),
        meta_key: meta_key.clone(),
        fps_limit: None,
    };
    let _zenoh_handle = tokio::spawn({
        let session = session.clone();
        let shared_meta = shared_meta.clone();
        let frame_tx = frame_tx.clone();
        let info_tx = info_tx.clone();
        let shutdown_rx = shutdown_rx.clone();
        async move {
            let _ = viva_streamer::zenoh_source::run_with_session(
                session,
                source_config,
                shared_meta,
                frame_tx,
                info_tx,
                shutdown_rx,
            )
            .await;
        }
    });

    // Spawn WS server task.
    let _ws_handle = tokio::spawn({
        let state = viva_streamer::ws::AppState {
            frame_tx: frame_tx.clone(),
            info_tx: info_tx.clone(),
        };
        let shutdown_rx = shutdown_rx.clone();
        async move {
            let _ = viva_streamer::ws::run_server_with_listener(
                listener,
                "/ws".to_string(),
                state,
                shutdown_rx,
            )
            .await;
        }
    });

    // Give the streamer tasks a moment to start.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 3. Publish image metadata on Zenoh.
    let meta = ImageMeta {
        pixel_format: PixelFormat::Mono8,
        width,
        height,
        payload_size: (width * height) as u64,
    };
    session
        .put(&meta_key, serde_json::to_vec(&meta).unwrap())
        .await
        .unwrap();

    // 4. Publish a synthetic Mono8 frame (gradient pattern).
    let pixel_count = (width * height) as usize;
    let mut pixels = vec![0u8; pixel_count];
    for (i, p) in pixels.iter_mut().enumerate() {
        *p = (i % 256) as u8;
    }
    let header = FrameHeader {
        pixel_format: PixelFormat::Mono8,
        width,
        height,
        seq: 0,
    };
    let encoded = header.encode();
    let mut payload = Vec::with_capacity(encoded.len() + pixels.len());
    payload.extend_from_slice(&encoded);
    payload.extend_from_slice(&pixels);

    session.put(&image_key, payload).await.unwrap();

    // Small delay for the streamer to process.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 5. Connect as WebSocket client and receive frames.
    let (mut ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("WS connect failed");

    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message;

    // First message should be the info frame (text).
    let msg = tokio::time::timeout(Duration::from_secs(3), ws_stream.next())
        .await
        .expect("timeout waiting for info frame")
        .expect("stream ended")
        .expect("WS error");

    match msg {
        Message::Text(text) => {
            let info: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(info["type"], "info");
            assert_eq!(info["width"], width);
            assert_eq!(info["height"], height);
            assert_eq!(info["encoding"], "BMP");
            tracing::info!("Got info frame: {text}");
        }
        other => panic!("Expected text info frame, got: {other:?}"),
    }

    // Publish another frame so the WS client gets a binary message.
    let header2 = FrameHeader {
        pixel_format: PixelFormat::Mono8,
        width,
        height,
        seq: 1,
    };
    let encoded2 = header2.encode();
    let mut payload2 = Vec::with_capacity(encoded2.len() + pixels.len());
    payload2.extend_from_slice(&encoded2);
    payload2.extend_from_slice(&pixels);
    session.put(&image_key, payload2).await.unwrap();

    // Wait for the binary BMP frame.
    let bmp_msg = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(Ok(Message::Binary(data))) = ws_stream.next().await {
                return data;
            }
        }
    })
    .await
    .expect("timeout waiting for BMP frame");

    // 6. Validate BMP header.
    assert!(bmp_msg.len() > 54, "BMP should be larger than header");
    // BMP magic bytes: 'B' 'M'
    assert_eq!(bmp_msg[0], b'B', "BMP magic byte 0");
    assert_eq!(bmp_msg[1], b'M', "BMP magic byte 1");

    // BMP width at offset 18 (4 bytes LE)
    let bmp_width = u32::from_le_bytes([bmp_msg[18], bmp_msg[19], bmp_msg[20], bmp_msg[21]]);
    assert_eq!(bmp_width, width, "BMP width should match");

    // BMP height at offset 22 (4 bytes LE, negative for top-down)
    let bmp_height = i32::from_le_bytes([bmp_msg[22], bmp_msg[23], bmp_msg[24], bmp_msg[25]]);
    assert_eq!(bmp_height.unsigned_abs(), height, "BMP height should match");

    tracing::info!(
        "Got BMP frame: {}x{} ({} bytes)",
        bmp_width,
        bmp_height.unsigned_abs(),
        bmp_msg.len()
    );

    // 7. Shutdown.
    let _ = shutdown_tx.send(true);
}
