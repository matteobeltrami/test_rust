use std::fmt;

/// Errors that can occur in server operations
#[derive(Debug)]
pub enum ServerError {
    NetworkError(String),
    SerializationError(String),
    ConfigurationError(String),
    FragmentationError(String),
    TimeoutError,
    ProtocolError(String),
    FileNotFound(String),
    UnknownServerType,
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ServerError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            ServerError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
            ServerError::FragmentationError(msg) => write!(f, "Fragmentation error: {}", msg),
            ServerError::TimeoutError => write!(f, "Operation timed out"),
            ServerError::ProtocolError(msg) => write!(f, "Protocol error: {}", msg),
            ServerError::FileNotFound(msg) => write!(f, "File not found: {}", msg),
            ServerError::UnknownServerType => write!(f, "Unknown server type"),
        }
    }
}

impl std::error::Error for ServerError {}