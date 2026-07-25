use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tracing::{debug, error, info};

use viva_zenoh_api::{
    AcquisitionCommand, AcquisitionControlRequest, AcquisitionStatus, FrameHeader, ImageMeta,
    NodeOpResponse, PixelFormat,
};

use crate::config::MockConfig;
use crate::state::NodeStore;

/// Generate an animated Mono8 gradient test pattern.
/// Brightness reacts to ExposureTime and Gain values.
fn generate_mono8(width: u32, height: u32, frame_id: u64, brightness_scale: f64) -> Vec<u8> {
    let mut pixels = vec![0u8; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let bar = ((x as u64 + frame_id * 2) % 256) as u8;
            let gradient = ((y as f32 / height as f32) * 255.0) as u8;
            let raw = bar.wrapping_add(gradient) as f64;
            let scaled = (raw * brightness_scale).clamp(0.0, 255.0) as u8;
            pixels[idx] = scaled;
        }
    }
    pixels
}

/// Generate an animated Mono16 gradient test pattern.
/// Output is width×height u16 values, little-endian byte order.
fn generate_mono16(width: u32, height: u32, frame_id: u64, brightness_scale: f64) -> Vec<u8> {
    let pixel_count = (width * height) as usize;
    let mut buf = vec![0u8; pixel_count * 2];
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let horiz = ((x as u64 + frame_id * 4) % 65536) as f64;
            let vert_phase = (y as f64 / height as f64) * std::f64::consts::TAU;
            let vert = (vert_phase.sin() * 0.25 + 0.75) * 65535.0;
            let raw = (horiz * 0.5 + vert * 0.5) * brightness_scale;
            let clamped = raw.clamp(0.0, 65535.0) as u16;
            let bytes = clamped.to_le_bytes();
            buf[idx * 2] = bytes[0];
            buf[idx * 2 + 1] = bytes[1];
        }
    }
    buf
}

/// Generate an animated BayerRG8 (RGGB) test pattern. One byte per pixel.
fn generate_bayer_rg8(width: u32, height: u32, frame_id: u64, brightness_scale: f64) -> Vec<u8> {
    let mut buf = vec![0u8; (width * height) as usize];
    let offset = (frame_id * 3) % 256;
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let raw: f64 = match (y % 2, x % 2) {
                (0, 0) => ((x as u64 + offset) % 256) as f64,
                (0, 1) | (1, 0) => (((x + y) as u64 + offset) % 256) as f64,
                _ => ((y as u64 + offset) % 256) as f64,
            };
            buf[idx] = (raw * brightness_scale).clamp(0.0, 255.0) as u8;
        }
    }
    buf
}

/// Generate an animated RGB8 test pattern. Three bytes per pixel (R, G, B).
fn generate_rgb8(width: u32, height: u32, frame_id: u64, brightness_scale: f64) -> Vec<u8> {
    let mut buf = vec![0u8; (width * height) as usize * 3];
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize * 3;
            let r = ((x as u64 + frame_id * 2) % 256) as f64;
            let g = ((y as u64 + frame_id * 3) % 256) as f64;
            let b = (((x + y) as u64 + frame_id) % 256) as f64;
            buf[idx] = (r * brightness_scale).clamp(0.0, 255.0) as u8;
            buf[idx + 1] = (g * brightness_scale).clamp(0.0, 255.0) as u8;
            buf[idx + 2] = (b * brightness_scale).clamp(0.0, 255.0) as u8;
        }
    }
    buf
}

/// Compute brightness scale from ExposureTime and Gain node values.
async fn compute_brightness(store: &NodeStore) -> f64 {
    // ExposureTime: default 10000us. Normalize so 10000 -> 1.0
    let exposure = store
        .get_value("ExposureTime")
        .await
        .and_then(|v| v.as_f64())
        .unwrap_or(10000.0);
    let exposure_factor = (exposure / 10000.0).clamp(0.01, 10.0);

    // Gain: default 1.0 dB. Each 6dB doubles brightness.
    let gain_db = store
        .get_value("Gain")
        .await
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let gain_factor = (10.0_f64).powf(gain_db / 20.0);

    exposure_factor * gain_factor
}

/// Get current image dimensions from the node store.
async fn get_dimensions(store: &NodeStore, config: &MockConfig) -> (u32, u32) {
    let width = store
        .get_value("Width")
        .await
        .and_then(|v| v.as_u64())
        .unwrap_or(config.width as u64) as u32;
    let height = store
        .get_value("Height")
        .await
        .and_then(|v| v.as_u64())
        .unwrap_or(config.height as u64) as u32;
    (width, height)
}

fn parse_pixel_format(s: &str) -> PixelFormat {
    // PixelFormat carries #[serde(other)] on Unknown, so deserialization never
    // returns Err for string inputs — the unwrap_or is a safety net for future changes.
    serde_json::from_value(serde_json::Value::String(s.to_string())).unwrap_or(PixelFormat::Unknown)
}

async fn publish_image_meta(
    session: &zenoh::Session,
    meta_key: &str,
    store: &NodeStore,
    config: &MockConfig,
) {
    let (width, height) = get_dimensions(store, config).await;
    let pf_str = store
        .get_value("PixelFormat")
        .await
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "Mono8".to_string());
    let pixel_format = parse_pixel_format(&pf_str);
    let payload_size = (width as f32 * height as f32 * pixel_format.bytes_per_pixel()) as u64;
    let meta = ImageMeta {
        pixel_format,
        width,
        height,
        payload_size,
    };
    match serde_json::to_vec(&meta) {
        Ok(bytes) => {
            let _ = session.put(meta_key, bytes).await;
        }
        Err(e) => {
            tracing::warn!("Failed to serialize ImageMeta: {e}");
        }
    }
}

pub async fn run(
    session: Arc<zenoh::Session>,
    config: MockConfig,
    store: Arc<NodeStore>,
    mut shutdown: watch::Receiver<bool>,
) {
    let control_key = viva_zenoh_api::keys::acquisition_control(&config.device_id);
    let status_key = viva_zenoh_api::keys::acquisition_status(&config.device_id);
    let image_key = viva_zenoh_api::keys::image(&config.device_id);
    let meta_key = viva_zenoh_api::keys::image_meta(&config.device_id);

    let queryable = match session.declare_queryable(&control_key).await {
        Ok(q) => q,
        Err(e) => {
            error!("Failed to declare acquisition control queryable: {e}");
            return;
        }
    };

    let (acq_tx, mut acq_rx) = watch::channel(false);
    let mut node_change_rx = store.subscribe();
    let mut frame_id: u64 = 0;

    // Publish initial status
    let initial_status = AcquisitionStatus {
        active: false,
        fps: None,
        dropped: 0,
    };
    let _ = session
        .put(
            &status_key,
            serde_json::to_vec(&initial_status).unwrap_or_default(),
        )
        .await;

    let frame_interval = Duration::from_secs_f64(1.0 / config.fps as f64);
    let mut ticker = tokio::time::interval(frame_interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() { break; }
            }
            query = queryable.recv_async() => {
                match query {
                    Ok(query) => {
                        let payload = query.payload()
                            .map(|p| p.to_bytes().to_vec())
                            .unwrap_or_default();
                        let request: Result<AcquisitionControlRequest, _> = serde_json::from_slice(&payload);
                        let resp = match request {
                            Ok(req) => {
                                match req.command {
                                    AcquisitionCommand::Start => {
                                        info!("Acquisition started");
                                        let _ = acq_tx.send(true);
                                        let status = AcquisitionStatus {
                                            active: true,
                                            fps: Some(config.fps),
                                            dropped: 0,
                                        };
                                        let _ = session.put(&status_key, serde_json::to_vec(&status).unwrap_or_default()).await;
                                        publish_image_meta(&session, &meta_key, &store, &config).await;
                                        NodeOpResponse { ok: true, error: None }
                                    }
                                    AcquisitionCommand::Stop => {
                                        info!("Acquisition stopped");
                                        let _ = acq_tx.send(false);
                                        let status = AcquisitionStatus {
                                            active: false,
                                            fps: None,
                                            dropped: 0,
                                        };
                                        let _ = session.put(&status_key, serde_json::to_vec(&status).unwrap_or_default()).await;
                                        NodeOpResponse { ok: true, error: None }
                                    }
                                }
                            }
                            Err(e) => {
                                NodeOpResponse { ok: false, error: Some(format!("Invalid request: {e}")) }
                            }
                        };
                        let _ = query.reply(&control_key, serde_json::to_vec(&resp).unwrap_or_default()).await;
                    }
                    Err(e) => {
                        error!("Acquisition control queryable recv error: {e}");
                        break;
                    }
                }
            }
            _ = ticker.tick(), if *acq_rx.borrow() => {
                let brightness = compute_brightness(&store).await;
                let (width, height) = get_dimensions(&store, &config).await;

                let pf_str = store
                    .get_value("PixelFormat")
                    .await
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_else(|| "Mono8".to_string());
                let pixel_format = parse_pixel_format(&pf_str);

                let pixels = match pixel_format {
                    PixelFormat::Mono16 => generate_mono16(width, height, frame_id, brightness),
                    PixelFormat::BayerRG8 => generate_bayer_rg8(width, height, frame_id, brightness),
                    PixelFormat::RGB8 => generate_rgb8(width, height, frame_id, brightness),
                    _ => generate_mono8(width, height, frame_id, brightness),
                };

                // Prepend the 16-byte self-describing frame header so the
                // streamer does not need to rely on image/meta subscription
                // ordering to know the frame dimensions and format.
                let seq = frame_id as u32;
                let header = FrameHeader {
                    pixel_format,
                    width,
                    height,
                    seq,
                };
                let mut payload = header.encode();
                payload.extend_from_slice(&pixels);

                frame_id = frame_id.wrapping_add(1);
                debug!(
                    "Publishing frame {frame_id}: {width}x{height}, format={pf_str}, brightness={brightness:.2}"
                );
                let _ = session.put(&image_key, payload).await;
            }
            _ = acq_rx.changed() => {
                // Acquisition state changed, loop will re-evaluate select conditions
            }
            result = node_change_rx.recv() => {
                match result {
                    Ok((name, _)) if *acq_rx.borrow() => {
                        if name == "Width" || name == "Height" || name == "PixelFormat" {
                            publish_image_meta(&session, &meta_key, &store, &config).await;
                        }
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Meta publisher lagged by {n} node change messages");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::warn!("Node change channel closed; stopping meta publisher");
                        break;
                    }
                }
            }
        }
    }
}
