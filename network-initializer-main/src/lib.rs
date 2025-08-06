mod errors;
mod parser;
mod utils;

#[cfg(test)]
mod tests {

    // TODO: "Implement more test for parse and validation logic"

    use super::*;
    use crate::parser::Parse;
    use crate::parser::Validate;
    use wg_internal::config::Config;
    use crate::errors::ConfigError;

    #[test]
    fn test_parse_config() {
        let config = Config::parse_config("./tests/correct_config.toml");
        assert!(config.is_ok());
    }

    #[test]
    fn test_validate_config() {
        let config = Config::parse_config("./tests/correct_config.toml").unwrap();
        let validation = config.validate_config();
        assert!(validation.is_ok());
    }

    #[test]
    fn test_unidirectional_error() {
        let config = Config::parse_config("./tests/unidirectional_error.toml").unwrap();
        let validation = config.validate_config();
        assert_eq!(validation, Err(ConfigError::UnidirectedConnection));
    }

    #[test]
    fn test_parsing_error() {
        let config = Config::parse_config("./tests/invalid_config.toml");
        assert!(config.is_err());
    }

    #[test]
    fn test_invalid_node_connection() {
        let config = Config::parse_config("./tests/invalid_node_connection1.toml").unwrap();
        let validation = config.validate_config();
        assert_eq!(validation, Err(ConfigError::InvalidNodeConnection("Drone 3 cannot be connected to itself".to_string())));
    }
}
