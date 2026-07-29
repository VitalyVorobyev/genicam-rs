//! Integration tests using the in-process fake GigE Vision camera.
//!
//! These tests require no external dependencies — `fake-gige` provides a
//! self-contained GVCP/GVSP camera on localhost.
//!
//! ```sh
//! cargo test -p genicam --test fake_camera
//! ```

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;
#[allow(clippy::single_component_path_imports)]
use viva_genapi_xml;
use viva_genicam::{Camera, GigeRegisterIo, connect_gige, connect_gige_with_xml, gige};

/// Helper: discover the fake camera via loopback.
async fn discover_fake() -> gige::DeviceInfo {
    let devices = gige::discover_all(Duration::from_secs(2))
        .await
        .expect("discovery failed");
    devices
        .into_iter()
        .find(|d| d.ip.is_loopback())
        .expect("fake camera not found on loopback")
}

/// Helper: connect to the fake camera, returning a shared handle safe for spawn_blocking.
async fn connect_fake() -> Arc<Mutex<Camera<GigeRegisterIo>>> {
    let device = discover_fake().await;
    let camera = connect_gige(&device).await.expect("connect failed");
    Arc::new(Mutex::new(camera))
}

/// Run a blocking camera operation from an async context.
async fn blocking_get(
    camera: &Arc<Mutex<Camera<GigeRegisterIo>>>,
    name: &str,
) -> Result<String, viva_genicam::GenicamError> {
    let cam = camera.clone();
    let name = name.to_string();
    tokio::task::spawn_blocking(move || {
        let cam = cam.lock().unwrap();
        cam.get(&name)
    })
    .await
    .unwrap()
}

async fn blocking_set(
    camera: &Arc<Mutex<Camera<GigeRegisterIo>>>,
    name: &str,
    value: &str,
) -> Result<(), viva_genicam::GenicamError> {
    let cam = camera.clone();
    let name = name.to_string();
    let value = value.to_string();
    tokio::task::spawn_blocking(move || {
        let mut cam = cam.lock().unwrap();
        cam.set(&name, &value)
    })
    .await
    .unwrap()
}

/// Resolve the loopback network interface (platform-independent).
fn loopback_iface() -> gige::nic::Iface {
    gige::nic::Iface::from_ipv4(std::net::Ipv4Addr::LOCALHOST).expect("loopback iface")
}

// ---------------------------------------------------------------------------
// Phase 1: Discovery & Connection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_discovery_finds_fake_camera() {
    let _cam = common::TestCamera::start().await;

    let devices = gige::discover_all(Duration::from_secs(2))
        .await
        .expect("discovery failed");

    assert!(!devices.is_empty(), "expected at least one device");
    let fake = devices
        .iter()
        .find(|d| d.ip.is_loopback())
        .expect("no loopback device found");

    // Assert the values, not merely that some field is populated. The previous
    // `model.is_some() || manufacturer.is_some()` disjunction passed happily
    // while the MAC was being read two bytes off (#57), and would have passed
    // with every string field empty. Per ADR-0019, identity assertions check
    // exactly what the fake was configured to report.
    assert_eq!(fake.mac, viva_fake_gige::FAKE_MAC, "MAC address mismatch");
    assert_eq!(
        fake.manufacturer.as_deref(),
        Some(viva_fake_gige::FAKE_MANUFACTURER)
    );
    assert_eq!(fake.model.as_deref(), Some(viva_fake_gige::FAKE_MODEL));
    assert_eq!(fake.version.as_deref(), Some(viva_fake_gige::FAKE_VERSION));
    assert_eq!(fake.serial.as_deref(), Some(viva_fake_gige::FAKE_SERIAL));
    assert_eq!(
        fake.user_name.as_deref(),
        Some(viva_fake_gige::FAKE_USER_NAME)
    );
}

/// Check the fake's Discovery ACK **bytes** against the specification's field
/// table, without going through `parse_discovery_payload`.
///
/// This is the ADR-0019 rule in practice. Routing the fake's output through our
/// own parser only proves the two agree with each other — which they did for
/// three separate wire bugs (the SCPS overhead, unaligned READMEM, and the MAC
/// offset in #57), each time while both disagreed with the standard.
#[tokio::test]
async fn test_fake_discovery_ack_matches_spec_layout() {
    use tokio::net::UdpSocket;

    let _cam = common::TestCamera::start().await;

    let socket = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
    let request_id: u16 = 0x0142;

    // GVCP command: 0x42 key, flags, opcode, length, request id.
    let mut cmd = Vec::new();
    cmd.push(0x42);
    cmd.push(0x11); // ACK_REQUIRED | BROADCAST
    cmd.extend_from_slice(&0x0002u16.to_be_bytes()); // DISCOVERY_CMD
    cmd.extend_from_slice(&0u16.to_be_bytes()); // no payload
    cmd.extend_from_slice(&request_id.to_be_bytes());
    socket
        .send_to(&cmd, ("127.0.0.1", gige::GVCP_PORT))
        .await
        .expect("send discovery");

    let mut buf = vec![0u8; 1024];
    let (len, _) = tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut buf))
        .await
        .expect("timed out waiting for discovery ack")
        .expect("recv");

    // 8-byte GVCP ack header.
    assert!(len >= 8 + 248, "ack shorter than a 248-byte payload: {len}");
    assert_eq!(u16::from_be_bytes([buf[0], buf[1]]), 0x0000, "status");
    assert_eq!(
        u16::from_be_bytes([buf[2], buf[3]]),
        0x0003,
        "DISCOVERY_ACK"
    );
    assert_eq!(u16::from_be_bytes([buf[4], buf[5]]), 248, "payload length");
    assert_eq!(u16::from_be_bytes([buf[6], buf[7]]), request_id);

    let p = &buf[8..8 + 248];
    let text = |at: usize, n: usize| {
        let field = &p[at..at + n];
        let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
        String::from_utf8_lossy(&field[..end]).to_string()
    };

    // Offsets from the specification's field table, corroborated by
    // Wireshark's dissect_discovery_ack().
    assert_eq!(&p[10..16], &viva_fake_gige::FAKE_MAC, "MAC at offset 10");
    assert_eq!(&p[36..40], &[127, 0, 0, 1], "current IP at offset 36");
    assert_eq!(text(72, 32), viva_fake_gige::FAKE_MANUFACTURER, "offset 72");
    assert_eq!(text(104, 32), viva_fake_gige::FAKE_MODEL, "offset 104");
    assert_eq!(text(136, 32), viva_fake_gige::FAKE_VERSION, "offset 136");
    assert_eq!(text(216, 16), viva_fake_gige::FAKE_SERIAL, "offset 216");
    assert_eq!(text(232, 16), viva_fake_gige::FAKE_USER_NAME, "offset 232");

    // Bytes 8..10 are the padding half of the MAC-high register. If a future
    // change reintroduces the old 4-byte skip, the MAC slides here.
    assert_eq!(&p[8..10], &[0, 0], "reserved MAC-high padding at offset 8");
}

#[tokio::test]
async fn test_connect_and_fetch_xml() {
    let _cam = common::TestCamera::start().await;

    let device = discover_fake().await;
    let (camera, xml) = connect_gige_with_xml(&device)
        .await
        .expect("connect failed");

    // XML should be non-empty and look like GenICam XML.
    assert!(!xml.is_empty(), "XML should not be empty");
    assert!(
        xml.contains("RegisterDescription") || xml.contains("Category"),
        "XML should contain GenICam elements"
    );

    // NodeMap should contain standard SFNC nodes.
    let nodemap = camera.nodemap();
    assert!(
        nodemap.node("Width").is_some(),
        "NodeMap should contain Width"
    );
    assert!(
        nodemap.node("Height").is_some(),
        "NodeMap should contain Height"
    );
}

/// A register whose address is `<Address>` + `<pIndex>` must resolve to the
/// sum, over the wire.
///
/// The GigE stream-channel block lives at `0x0D00 + channel * 0x40`, so
/// `GevSCPSPacketSize` is a fixed offset plus a scaled index. Dropping the
/// `<pIndex>` term would still return a plausible value — channel 0's — which
/// is why this needs an end-to-end check rather than a unit test.
#[tokio::test]
async fn test_summed_address_resolves_over_the_wire() {
    let _cam = common::TestCamera::start().await;
    let camera = connect_fake().await;

    // Channel 0 lives at 0x0D04, channel 1 at 0x0D44, and the fake camera gives
    // them different packet sizes. An ignored <pIndex> term would return
    // channel 0's value both times.
    let channel0 = blocking_get(&camera, "GevSCPSPacketSize")
        .await
        .expect("read channel 0 packet size");
    assert_eq!(channel0, "1500");

    blocking_set(&camera, "GevStreamChannelSelector", "1")
        .await
        .expect("select channel 1");
    let channel1 = blocking_get(&camera, "GevSCPSPacketSize")
        .await
        .expect("read channel 1 packet size");
    assert_eq!(
        channel1, "9000",
        "the <pIndex> term did not move the address"
    );
}

/// `<StructReg>` bits addressed through `<pAddress>` + `<Address>` read the
/// right register, and read as 1 rather than -1.
#[tokio::test]
async fn test_struct_reg_inquiry_bits_over_the_wire() {
    let _cam = common::TestCamera::start().await;
    let camera = connect_fake().await;

    // The fake camera reports 0xC0000000: capability bits 0 and 1 set, 2 clear.
    for (bit, expected) in [
        ("FrameRateControlInq_Bit", "1"),
        ("ChunkSupportInq_Bit", "1"),
        ("SequencerInq_Bit", "0"),
    ] {
        let value = blocking_get(&camera, bit).await.expect("read inquiry bit");
        assert_eq!(value, expected, "{bit} read as {value}");
    }
}

#[tokio::test]
async fn test_connect_with_zipped_xml() {
    // Many real cameras (Basler, FLIR, Hikrobot, ...) serve their GenApi XML
    // as a ZIP archive; issue #35 hit exactly this path. The archive length
    // is also virtually never 4-byte aligned, which exercises the READMEM
    // alignment handling against the strict fake GVCP server.
    let _cam = common::TestCamera::start_with(|builder| builder.zip_xml(true)).await;

    let device = discover_fake().await;
    let (camera, xml) = connect_gige_with_xml(&device)
        .await
        .expect("connect with zipped XML failed");

    assert!(
        xml.contains("RegisterDescription"),
        "decompressed XML should contain GenICam elements"
    );
    let nodemap = camera.nodemap();
    assert!(
        nodemap.node("Width").is_some(),
        "NodeMap should contain Width"
    );
}

#[tokio::test]
async fn test_connect_with_cdata_tooltip() {
    // Issue #45: a FLIR BFS-PGE camera could not be opened because a CDATA
    // section in a text element was run through XML unescaping, which chokes on
    // the literal `&` that is legal there. Connecting must succeed and the
    // tooltip must survive intact.
    let _cam = common::TestCamera::start().await;

    let device = discover_fake().await;
    let camera = connect_gige(&device).await.expect("connect failed");

    let tooltip = camera
        .nodemap()
        .node("Gain")
        .expect("NodeMap should contain Gain")
        .tooltip()
        .expect("Gain should have a tooltip");
    assert!(
        tooltip.contains("0 < gain & gain < 48"),
        "CDATA tooltip should be preserved verbatim, got: {tooltip}"
    );
}

#[tokio::test]
async fn test_claim_control_visible_via_register_read() {
    let _cam = common::TestCamera::start().await;

    let device_info = discover_fake().await;
    use std::net::{IpAddr, SocketAddr};

    let control_addr = SocketAddr::new(IpAddr::V4(device_info.ip), gige::GVCP_PORT);
    let mut device = gige::GigeDevice::open(control_addr)
        .await
        .expect("open device");

    device.claim_control().await.expect("claim CCP");

    let privilege = device
        .read_register(gige::gvcp::consts::CONTROL_CHANNEL_PRIVILEGE as u32)
        .await
        .expect("read CCP register");
    let controller_bits = gige::gvcp::consts::CCP_CONTROL | gige::gvcp::consts::CCP_EXCLUSIVE;
    assert_ne!(
        privilege & controller_bits,
        0,
        "CCP register should report an active controller, got 0x{privilege:08x}"
    );

    device.release_control().await.expect("release CCP");
}

// ---------------------------------------------------------------------------
// Phase 2: Feature Read / Write
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_read_width_height() {
    let _cam = common::TestCamera::start().await;
    let camera = connect_fake().await;

    let width = blocking_get(&camera, "Width").await.expect("read Width");
    let height = blocking_get(&camera, "Height").await.expect("read Height");

    let w: i64 = width.parse().expect("Width should be an integer");
    let h: i64 = height.parse().expect("Height should be an integer");
    assert!(w > 0, "Width should be positive, got {w}");
    assert!(h > 0, "Height should be positive, got {h}");
}

#[tokio::test]
async fn test_read_pixel_format() {
    let _cam = common::TestCamera::start().await;
    let camera = connect_fake().await;

    let pf = blocking_get(&camera, "PixelFormat")
        .await
        .expect("read PixelFormat");
    assert!(
        !pf.is_empty(),
        "PixelFormat should return a non-empty string"
    );
}

#[tokio::test]
async fn test_read_exposure_time() {
    let _cam = common::TestCamera::start().await;
    let camera = connect_fake().await;

    let exp = blocking_get(&camera, "ExposureTime")
        .await
        .expect("read ExposureTime");

    let v: f64 = exp.parse().expect("ExposureTime should be a float");
    // Fake camera seeds ExposureTime at 5000.0 µs as IEEE 754 f64.
    // Prior to the Float-encoding fix this came back as `4662219572839973000`
    // (the f64 bit pattern of 5000.0 interpreted as i64).
    assert!(
        (v - 5000.0).abs() < 1.0,
        "ExposureTime should be ≈ 5000.0 µs, got {v}"
    );
}

#[tokio::test]
async fn test_read_acquisition_frame_rate() {
    let _cam = common::TestCamera::start().await;
    let camera = connect_fake().await;

    let rate = blocking_get(&camera, "AcquisitionFrameRate")
        .await
        .expect("read AcquisitionFrameRate");

    let v: f64 = rate
        .parse()
        .expect("AcquisitionFrameRate should be a float");
    // Fake camera seeds AcquisitionFrameRate at 30.0 fps as IEEE 754 f32.
    // Prior to the Float-encoding fix this came back as `1106247680`.
    assert!(
        (v - 30.0).abs() < 1e-3,
        "AcquisitionFrameRate should be ≈ 30.0 fps, got {v}"
    );
}

#[tokio::test]
async fn test_exposure_time_roundtrip() {
    let _cam = common::TestCamera::start().await;
    let camera = connect_fake().await;

    blocking_set(&camera, "ExposureTime", "7500.0")
        .await
        .expect("set ExposureTime");

    let exp = blocking_get(&camera, "ExposureTime")
        .await
        .expect("read ExposureTime");
    let v: f64 = exp.parse().expect("ExposureTime should be a float");
    assert!((v - 7500.0).abs() < 1.0, "got {v}");

    blocking_set(&camera, "ExposureTime", "5000.0")
        .await
        .expect("restore ExposureTime");
}

#[tokio::test]
async fn test_set_and_readback_width() {
    let _cam = common::TestCamera::start().await;
    let camera = connect_fake().await;

    // Read current width first.
    let original = blocking_get(&camera, "Width").await.expect("read Width");
    let original_val: i64 = original.parse().unwrap();

    // Set to a different valid value.
    let new_val = if original_val > 128 { 128 } else { 256 };
    blocking_set(&camera, "Width", &new_val.to_string())
        .await
        .expect("set Width");

    let readback = blocking_get(&camera, "Width")
        .await
        .expect("readback Width");
    let readback_val: i64 = readback.parse().unwrap();
    assert_eq!(
        readback_val, new_val,
        "Width readback should match set value"
    );

    // Restore original.
    blocking_set(&camera, "Width", &original)
        .await
        .expect("restore Width");
}

#[tokio::test]
async fn test_exec_acquisition_commands() {
    let _cam = common::TestCamera::start().await;
    let camera = connect_fake().await;

    let cam = camera.clone();
    tokio::task::spawn_blocking(move || {
        let mut cam = cam.lock().unwrap();
        cam.acquisition_start().expect("acquisition_start");
        cam.acquisition_stop().expect("acquisition_stop");
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn test_read_nonexistent_node() {
    let _cam = common::TestCamera::start().await;
    let camera = connect_fake().await;

    let result = blocking_get(&camera, "NonExistentNode12345").await;
    assert!(result.is_err(), "reading nonexistent node should fail");
}

// ---------------------------------------------------------------------------
// Phase 3: Streaming
// ---------------------------------------------------------------------------

/// Helper: set up a streaming session.
async fn setup_stream(
    device_info: &gige::DeviceInfo,
) -> (
    viva_genicam::FrameStream,
    Arc<Mutex<Camera<GigeRegisterIo>>>,
) {
    use std::net::{IpAddr, SocketAddr};

    let control_addr = SocketAddr::new(IpAddr::V4(device_info.ip), gige::GVCP_PORT);

    // Fetch XML via a temporary connection.
    let xml = viva_genapi_xml::fetch_and_load_xml({
        let addr = control_addr;
        move |address, length| {
            let addr = addr;
            async move {
                let mut dev = gige::GigeDevice::open(addr)
                    .await
                    .map_err(|e| viva_genapi_xml::XmlError::Transport(e.to_string()))?;
                dev.read_mem(address, length)
                    .await
                    .map_err(|e| viva_genapi_xml::XmlError::Transport(e.to_string()))
            }
        }
    })
    .await
    .expect("fetch XML");

    let model = viva_genapi_xml::parse(&xml).expect("parse XML");
    let nodemap = viva_genicam::genapi::NodeMap::try_from_xml(model).expect("build nodemap");

    // Main device: claim CCP, configure stream.
    let mut device = gige::GigeDevice::open(control_addr)
        .await
        .expect("open device");
    device.claim_control().await.expect("claim control");

    let iface = loopback_iface();
    let stream = viva_genicam::StreamBuilder::new(&mut device)
        .iface(iface)
        .auto_packet_size(false)
        .build()
        .await
        .expect("build stream");
    let frame_stream = viva_genicam::FrameStream::new(stream, None);

    let handle = tokio::runtime::Handle::current();
    let transport = GigeRegisterIo::new(handle, device);
    let camera = Arc::new(Mutex::new(Camera::new(transport, nodemap)));

    (frame_stream, camera)
}

#[tokio::test]
async fn test_stream_receives_frames() {
    let _cam = common::TestCamera::start().await;
    let device_info = discover_fake().await;

    let (mut frame_stream, camera) = setup_stream(&device_info).await;

    // Start acquisition.
    let cam = camera.clone();
    tokio::task::spawn_blocking(move || {
        let mut cam = cam.lock().unwrap();
        cam.acquisition_start().expect("start acquisition");
    })
    .await
    .unwrap();

    // Receive at least one frame.
    let frame = tokio::time::timeout(Duration::from_secs(5), frame_stream.next_frame())
        .await
        .expect("timeout waiting for frame")
        .expect("frame error")
        .expect("stream ended without a frame");

    assert!(frame.width > 0, "frame width should be positive");
    assert!(frame.height > 0, "frame height should be positive");

    // Mono8: the payload must be exactly width*height bytes. A mismatch means
    // the reassembly stride disagrees with the sender's packet chunking
    // (e.g. GevSCPSPacketSize header-overhead accounting).
    let expected_len = (frame.width * frame.height) as usize;
    assert_eq!(
        frame.payload.len(),
        expected_len,
        "frame payload length must match width*height for Mono8"
    );
    // The test pattern is non-constant; an all-identical payload means the
    // image data never actually landed in the buffer.
    let first = frame.payload[0];
    assert!(
        frame.payload.iter().any(|&b| b != first),
        "frame payload should contain a non-constant test pattern"
    );

    let cam = camera.clone();
    tokio::task::spawn_blocking(move || {
        let mut cam = cam.lock().unwrap();
        cam.acquisition_stop().expect("stop acquisition");
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn test_frame_dimensions_match() {
    let _cam = common::TestCamera::start().await;
    let device_info = discover_fake().await;

    let (mut frame_stream, camera) = setup_stream(&device_info).await;

    // Read expected dimensions.
    let expected_w: u32 = blocking_get(&camera, "Width")
        .await
        .expect("Width")
        .parse()
        .unwrap();
    let expected_h: u32 = blocking_get(&camera, "Height")
        .await
        .expect("Height")
        .parse()
        .unwrap();

    let cam = camera.clone();
    tokio::task::spawn_blocking(move || {
        let mut cam = cam.lock().unwrap();
        cam.acquisition_start().expect("start");
    })
    .await
    .unwrap();

    let frame = tokio::time::timeout(Duration::from_secs(5), frame_stream.next_frame())
        .await
        .expect("timeout")
        .expect("frame error")
        .expect("stream ended without a frame");

    assert_eq!(frame.width, expected_w, "frame width mismatch");
    assert_eq!(frame.height, expected_h, "frame height mismatch");
    assert_eq!(
        frame.payload.len(),
        (expected_w * expected_h) as usize,
        "Mono8 payload length must match Width*Height"
    );

    let cam = camera.clone();
    tokio::task::spawn_blocking(move || {
        let mut cam = cam.lock().unwrap();
        cam.acquisition_stop().expect("stop");
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn test_full_lifecycle() {
    let _cam = common::TestCamera::start().await;
    let device_info = discover_fake().await;

    let (mut frame_stream, camera) = setup_stream(&device_info).await;

    let cam = camera.clone();
    tokio::task::spawn_blocking(move || {
        let mut cam = cam.lock().unwrap();
        cam.acquisition_start().expect("start");
    })
    .await
    .unwrap();

    // Receive 5 frames.
    for i in 0..5 {
        let frame = tokio::time::timeout(Duration::from_secs(5), frame_stream.next_frame())
            .await
            .unwrap_or_else(|_| panic!("timeout on frame {i}"))
            .unwrap_or_else(|e| panic!("error on frame {i}: {e}"))
            .unwrap_or_else(|| panic!("stream ended on frame {i}"));
        assert!(frame.width > 0);
        assert!(frame.height > 0);
        assert_eq!(
            frame.payload.len(),
            (frame.width * frame.height) as usize,
            "Mono8 payload length must match width*height on frame {i}"
        );
    }

    let cam = camera.clone();
    tokio::task::spawn_blocking(move || {
        let mut cam = cam.lock().unwrap();
        cam.acquisition_stop().expect("stop");
    })
    .await
    .unwrap();

    drop(frame_stream);
    drop(camera);
}

// ---------------------------------------------------------------------------
// Phase 4: IP management
// ---------------------------------------------------------------------------

/// Helper: open a `GigeDevice` control connection to the fake camera.
async fn open_fake_device(device_info: &gige::DeviceInfo) -> gige::GigeDevice {
    use std::net::{IpAddr, SocketAddr};
    let addr = SocketAddr::new(IpAddr::V4(device_info.ip), gige::GVCP_PORT);
    gige::GigeDevice::open(addr).await.expect("open GigeDevice")
}

#[tokio::test]
async fn test_persistent_ip_roundtrip() {
    let _cam = common::TestCamera::start().await;
    let device_info = discover_fake().await;

    let mut device = open_fake_device(&device_info).await;
    device.claim_control().await.expect("claim control");

    let ip: std::net::Ipv4Addr = "192.168.10.50".parse().unwrap();
    let subnet: std::net::Ipv4Addr = "255.255.255.0".parse().unwrap();
    let gateway: std::net::Ipv4Addr = "192.168.10.1".parse().unwrap();

    device
        .write_persistent_ip(ip, subnet, gateway)
        .await
        .expect("write_persistent_ip");

    let (read_ip, read_subnet, read_gateway) = device
        .read_persistent_ip()
        .await
        .expect("read_persistent_ip");

    assert_eq!(read_ip, ip, "persistent IP roundtrip mismatch");
    assert_eq!(read_subnet, subnet, "persistent subnet roundtrip mismatch");
    assert_eq!(
        read_gateway, gateway,
        "persistent gateway roundtrip mismatch"
    );

    device
        .enable_persistent_ip()
        .await
        .expect("enable_persistent_ip");

    device.release_control().await.expect("release control");
}

// NOTE: FORCEIP integration test removed -- UDP broadcast from a loopback-bound
// socket does not reliably reach the fake camera across platforms (fails on macOS
// outright, times out on some Linux CI runners). The FORCEIP payload encoding is
// validated by the unit test `forceip_payload_encoding` in viva-gige/src/gvcp.rs.
