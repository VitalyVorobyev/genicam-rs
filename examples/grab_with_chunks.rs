use genicam::{parse_chunk_bytes, ChunkKind, ChunkValue};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    // In a real application chunk bytes come from the GVSP trailer.
    let mut raw = Vec::new();
    raw.extend_from_slice(&0x0001u16.to_be_bytes());
    raw.extend_from_slice(&0u16.to_be_bytes());
    raw.extend_from_slice(&8u32.to_be_bytes());
    raw.extend_from_slice(&0x0102_0304_0506_0708u64.to_be_bytes());
    raw.extend_from_slice(&0x0002u16.to_be_bytes());
    raw.extend_from_slice(&0u16.to_be_bytes());
    raw.extend_from_slice(&8u32.to_be_bytes());
    raw.extend_from_slice(&1234.5f64.to_be_bytes());
    let chunks = parse_chunk_bytes(&raw)?;
    for (kind, value) in chunks.iter() {
        match (kind, value) {
            (ChunkKind::Timestamp, ChunkValue::Timestamp(ts)) => {
                println!("Chunk Timestamp: {ts}");
            }
            (ChunkKind::ExposureTime, ChunkValue::ExposureTime(v)) => {
                println!("Exposure Time: {v} us");
            }
            (other_kind, other_value) => {
                println!("{other_kind:?}: {other_value:?}");
            }
        }
    }
    Ok(())
}
