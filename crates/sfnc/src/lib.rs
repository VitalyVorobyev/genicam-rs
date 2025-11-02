//! Standard Feature Naming Convention (SFNC) helpers.

#![allow(dead_code)]

/// Exposure time feature name (`ExposureTime`).
pub const EXPOSURE_TIME: &str = "ExposureTime";
/// Gain feature name (`Gain`).
pub const GAIN: &str = "Gain";
/// Gain selector feature name (`GainSelector`).
pub const GAIN_SELECTOR: &str = "GainSelector";
/// Pixel format feature name (`PixelFormat`).
pub const PIXEL_FORMAT: &str = "PixelFormat";
/// Chunk mode enable feature name (`ChunkModeActive`).
pub const CHUNK_MODE_ACTIVE: &str = "ChunkModeActive";
/// Chunk selector enumeration feature name (`ChunkSelector`).
pub const CHUNK_SELECTOR: &str = "ChunkSelector";
/// Chunk enable boolean feature name (`ChunkEnable`).
pub const CHUNK_ENABLE: &str = "ChunkEnable";
/// Acquisition start command feature name (`AcquisitionStart`).
pub const ACQUISITION_START: &str = "AcquisitionStart";
/// Acquisition stop command feature name (`AcquisitionStop`).
pub const ACQUISITION_STOP: &str = "AcquisitionStop";
/// Acquisition mode enumeration feature name (`AcquisitionMode`).
pub const ACQUISITION_MODE: &str = "AcquisitionMode";
/// Device temperature float feature name (`DeviceTemperature`).
pub const DEVICE_TEMPERATURE: &str = "DeviceTemperature";
