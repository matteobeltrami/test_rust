use common::network::NetworkError;


#[derive(Debug)]
pub enum ClientError {
    NetworkError(NetworkError),
    FragmentationError(String),
    SerializationError(String),
    ProtocolError(String),
    TimeoutError,
    UnknownServer,
    InvalidResponse,
    InvalidClient
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::NetworkError(msg) => write!(f, "Net ork error: {msg}"),
            ClientError::FragmentationError(msg) => write!(f, "Fragmentation error: {msg}"),
            ClientError::SerializationError(msg) => write!(f, "Serialization error: {msg}"),
            ClientError::ProtocolError(msg) => write!(f, "Protocol error: {msg}"),
            ClientError::TimeoutError => write!(f, "Operation timed out"),
            ClientError::UnknownServer => write!(f, "Unknown server"),
            ClientError::InvalidResponse => write!(f, "Invalid response from server"),
            ClientError::InvalidClient => write!(f, "Invalid client, missing attributes"),
        }
    }
}

impl std::error::Error for ClientError {}


impl From<NetworkError> for ClientError {
    fn from(value: NetworkError) -> Self {
        ClientError::NetworkError(value)
    }
}
