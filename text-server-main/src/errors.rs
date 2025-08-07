use std::fmt;

/// Error types specific to server operations
#[derive(Debug)]
pub enum ServerError {
    NetworkError(String),
    SerializationError(String),
    ConfigurationError(String),
    FileNotFound(String),
    UnsupportedOperation(String),
    ValidationError(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ServerError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            ServerError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
            ServerError::FileNotFound(msg) => write!(f, "File not found: {}", msg),
            ServerError::UnsupportedOperation(msg) => write!(f, "Unsupported operation: {}", msg),
            ServerError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for ServerError {}

impl From<Box<dyn std::error::Error>> for ServerError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        ServerError::NetworkError(err.to_string())
    }
}