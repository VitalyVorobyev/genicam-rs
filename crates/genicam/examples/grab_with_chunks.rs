use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use bytes::BytesMut;
use genicam::genapi::NodeMap;
use genicam::gige::gvsp::{self, GvspPacket};
use genicam::gige::nic::Iface;
use genicam::gige::GVCP_PORT;
use genicam::{
    parse_chunk_bytes, Camera, ChunkConfig, ChunkKind, ChunkValue, Frame, GenicamError,
    GigeRegisterIo, StreamBuilder,
};
use tokio::sync::Mutex;
use tracing::warn;

#[derive(Debug, Default)]
struct Args {
    iface: Option<String>,
}

fn print_usage() {
    eprintln!("usage: grab_with_chunks --iface <name>");
}

fn parse_args() -> Result<Args, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut parsed = Args::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--iface" => {
                let name = args
                    .next()
                    .ok_or_else(|| "--iface requires an interface name".to_string())?;
                parsed.iface = Some(name);
            }
            "--help" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }
    Ok(parsed)
}

#[derive(Debug)]
struct BlockState {
    block_id: u16,
    payload: BytesMut,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    let args = parse_args()?;
    let iface_name = match args.iface.as_deref() {
        Some(name) => name,
        None => {
            println!("Please specify the capture interface using --iface <name>.");
            print_usage();
            return Ok(());
        }
    };

    let iface = Iface::from_system(iface_name)?;
    let timeout = Duration::from_millis(500);
    let mut devices = genicam::gige::discover(timeout).await?;
    if devices.is_empty() {
        println!("No GigE Vision devices discovered.");
        return Ok(());
    }
    let device = devices.remove(0);
    println!(
        "Connecting to {} on interface {}",
        device.model.clone().unwrap_or_else(|| "camera".to_string()),
        iface.name()
    );

    let control_addr = SocketAddr::new(IpAddr::V4(device.ip), GVCP_PORT);
    let control = std::sync::Arc::new(Mutex::new(
        genicam::gige::GigeDevice::open(control_addr).await?,
    ));
    let xml = genapi_xml::fetch_and_load_xml({
        let control = control.clone();
        move |address, length| {
            let control = control.clone();
            async move {
                let mut dev = control.lock().await;
                dev.read_mem(address, length)
                    .await
                    .map_err(|err| genapi_xml::XmlError::Transport(err.to_string()))
            }
        }
    })
    .await?;
    let model = genapi_xml::parse(&xml)?;
    let nodemap = NodeMap::from(model);
    let handle = tokio::runtime::Handle::current();
    let control_device = match std::sync::Arc::try_unwrap(control) {
        Ok(mutex) => mutex.into_inner(),
        Err(_) => return Err("control connection still in use".into()),
    };
    let transport = GigeRegisterIo::new(handle.clone(), control_device);
    let mut camera = Camera::new(transport, nodemap);

    let selectors = match camera.enum_entries(sfnc::CHUNK_SELECTOR) {
        Ok(entries) => entries,
        Err(err) => {
            println!("ChunkSelector enumeration not available: {err}");
            return Ok(());
        }
    };
    let desired = ["Timestamp", "ExposureTime"];
    let mut enable_selectors = Vec::new();
    for wanted in desired {
        if selectors.iter().any(|entry| entry == wanted) {
            enable_selectors.push(wanted.to_string());
        } else {
            println!("Selector '{wanted}' not provided by this camera; skipping.");
        }
    }
    if enable_selectors.is_empty() {
        println!("No compatible chunk selectors available; exiting.");
        return Ok(());
    }

    let cfg = ChunkConfig {
        selectors: enable_selectors.clone(),
        active: true,
    };
    if let Err(err) = camera.configure_chunks(&cfg) {
        match err {
            GenicamError::MissingChunkFeature(name) => {
                println!(
                    "Missing required chunk feature '{name}'. Ensure the camera supports ChunkModeActive."
                );
                return Ok(());
            }
            GenicamError::GenApi(inner) => {
                println!("Failed to enable chunks via GenApi: {inner}");
                return Ok(());
            }
            other => return Err(other.into()),
        }
    }
    println!("Chunk mode enabled for selectors: {:?}", enable_selectors);

    let mut stream_device = genicam::gige::GigeDevice::open(control_addr).await?;
    let stream = StreamBuilder::new(&mut stream_device)
        .iface(iface.clone())
        .build()
        .await?;

    camera.acquisition_start()?;
    let packet_budget = stream.params().packet_size as usize + 64;
    let mut recv_buffer = vec![0u8; packet_budget.max(4096)];
    let mut frames_remaining = 5usize;
    let mut state: Option<BlockState> = None;
    let mut frame_index = 0usize;

    while frames_remaining > 0 {
        let (len, _) = stream.socket().recv_from(&mut recv_buffer).await?;
        let packet = match gvsp::parse_packet(&recv_buffer[..len]) {
            Ok(packet) => packet,
            Err(err) => {
                warn!(error = %err, "discarding malformed GVSP packet");
                continue;
            }
        };
        match packet {
            GvspPacket::Leader { block_id, .. } => {
                state = Some(BlockState {
                    block_id,
                    payload: BytesMut::new(),
                });
            }
            GvspPacket::Payload { block_id, data, .. } => {
                if let Some(active) = state.as_mut() {
                    if active.block_id == block_id {
                        active.payload.extend_from_slice(data.as_ref());
                    }
                }
            }
            GvspPacket::Trailer {
                block_id,
                status,
                chunk_data,
                ..
            } => {
                let Some(active) = state.take() else { continue };
                if active.block_id != block_id {
                    continue;
                }
                if status != 0 {
                    warn!(block_id, status, "trailer reported non-zero status");
                }
                let chunk_map = match parse_chunk_bytes(chunk_data.as_ref()) {
                    Ok(map) => map,
                    Err(err) => {
                        warn!(block_id, error = %err, "failed to decode chunk payload");
                        HashMap::new()
                    }
                };
                let frame = Frame {
                    payload: active.payload.freeze(),
                    chunks: if chunk_map.is_empty() {
                        None
                    } else {
                        Some(chunk_map)
                    },
                };
                frame_index += 1;
                print_frame_summary(frame_index, &frame);
                frames_remaining -= 1;
            }
        }
    }

    camera.acquisition_stop()?;
    println!("Capture complete.");
    Ok(())
}

fn print_frame_summary(index: usize, frame: &Frame) {
    println!("Frame #{index}: {} bytes payload", frame.payload.len());
    match frame.chunk(ChunkKind::Timestamp) {
        Some(ChunkValue::U64(ts)) => println!("  Timestamp: {ts}"),
        _ => println!("  Timestamp: <not available>"),
    }
    match frame.chunk(ChunkKind::ExposureTime) {
        Some(ChunkValue::F64(exposure)) => println!("  ExposureTime: {exposure:.3} us"),
        _ => println!("  ExposureTime: <not available>"),
    }
    if let Some(chunks) = frame.chunks.as_ref() {
        for (kind, value) in chunks {
            if matches!(kind, ChunkKind::Timestamp | ChunkKind::ExposureTime) {
                continue;
            }
            match value {
                ChunkValue::U32(bits) => println!("  {kind:?}: 0x{bits:08X}"),
                ChunkValue::U64(value) => println!("  {kind:?}: {value}"),
                ChunkValue::F64(value) => println!("  {kind:?}: {value}"),
                ChunkValue::Bytes(bytes) => println!("  {kind:?}: {} raw bytes", bytes.len()),
            }
        }
    } else {
        println!("  No chunk data reported.");
    }
}
