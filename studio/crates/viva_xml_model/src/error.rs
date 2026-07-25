#[derive(Debug)]
pub enum ParseError {
    Xml(String),
    GenApi(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Xml(msg) => write!(f, "XML parse error: {msg}"),
            Self::GenApi(msg) => write!(f, "GenAPI error: {msg}"),
        }
    }
}

impl std::error::Error for ParseError {}
