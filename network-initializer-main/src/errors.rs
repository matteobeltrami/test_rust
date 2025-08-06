use clap::Parser;

#[derive(Debug)]
pub enum ConfigError {
    InvalidConfig(String),
    ParseError(String),
    ConfigNotFound(String),
    EmptyPath,
    InvalidNodeConnection(String),
    DuplicateNodeId(String),
    InvalidPdrValue,
    UnidirectedConnection,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::EmptyPath => write!(f, "The provided path is empty."),
            ConfigError::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
            ConfigError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ConfigError::ConfigNotFound(name) => write!(f, "Configuration not found: {}", name),
            ConfigError::InvalidNodeConnection(msg) => {
                write!(f, "Invalid node connection: {}", msg)
            }
            ConfigError::DuplicateNodeId(msg) => write!(f, "Duplicate node ID found: {}", msg),
            ConfigError::InvalidPdrValue => write!(f, "Invalid PDR value provided."),
            ConfigError::UnidirectedConnection => {
                write!(f, "Unidirected connection is not allowed.")
            }
        }
    }
}

impl PartialEq<Self> for ConfigError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ConfigError::InvalidConfig(msg1), ConfigError::InvalidConfig(msg2)) => msg1 == msg2,
            (ConfigError::ParseError(msg1), ConfigError::ParseError(msg2)) => msg1 == msg2,
            (ConfigError::ConfigNotFound(name1), ConfigError::ConfigNotFound(name2)) => name1 == name2,
            (ConfigError::EmptyPath, ConfigError::EmptyPath) => true,
            (ConfigError::InvalidNodeConnection(msg1), ConfigError::InvalidNodeConnection(msg2)) => msg1 == msg2,
            (ConfigError::DuplicateNodeId(msg1), ConfigError::DuplicateNodeId(msg2)) => msg1 == msg2,
            (ConfigError::InvalidPdrValue, ConfigError::InvalidPdrValue) => true,
            (ConfigError::UnidirectedConnection, ConfigError::UnidirectedConnection) => true,
            _ => false,
        }
    }
}
