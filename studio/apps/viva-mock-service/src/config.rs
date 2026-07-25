#[derive(Debug, Clone)]
pub struct MockConfig {
    pub device_id: String,
    pub device_name: String,
    pub model: String,
    pub serial: String,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
}

impl MockConfig {
    pub fn new(
        device_id: impl Into<String>,
        device_name: impl Into<String>,
        model: impl Into<String>,
        serial: impl Into<String>,
        width: u32,
        height: u32,
        fps: f32,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            device_name: device_name.into(),
            model: model.into(),
            serial: serial.into(),
            width,
            height,
            fps,
        }
    }
}
