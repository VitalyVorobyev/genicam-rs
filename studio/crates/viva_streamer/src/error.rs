use thiserror::Error;

#[derive(Debug, Error)]
pub enum StreamerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Zenoh error: {0}")]
    Zenoh(#[from] zenoh::Error),

    #[error("BMP encode error: {0}")]
    Bmp(#[from] crate::bmp::BmpError),

    #[error("Invalid configuration: {0}")]
    Config(String),

    #[error("WebSocket server error: {0}")]
    Server(String),
}
