mod errors;
pub mod network_initializer;
mod parser;
#[macro_use]
mod utils;

#[cfg(test)]
mod tests {

    use crate::errors::ConfigError;
    use crate::network_initializer::NetworkInitializer;
    use crate::network_initializer::Uninitialized;
    use crate::parser::Parse;
    use crate::parser::Validate;
    use common::types::NodeCommand;
    use wg_internal::config::Config;

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
        assert_eq!(
            validation,
            Err(ConfigError::InvalidNodeConnection(
                "Drone 3 cannot be connected to itself".to_string()
            ))
        );
    }

    #[test]
    fn test_network_initializer() {
        let net_init = NetworkInitializer::<Uninitialized>::new("./tests/correct_config.toml");
        let _net_init = net_init.initialize();

        println!("Initialized!");
    }

    #[test]
    fn test_getters_after_running() {
        let config_path = "./config/butterfly.toml";
        let mut running = NetworkInitializer::<Uninitialized>::new(config_path)
            .initialize()
            .start_simulation();

        // Check clients, servers, drones maps
        let drones = running.get_drones();
        let clients = running.get_clients();
        let servers = running.get_servers();

        assert!(!clients.is_empty(), "Clients should not be empty");
        assert!(!servers.is_empty(), "Servers should not be empty");

        // Drones may be optional depending on config
        // but if present, check channels are usable
        for (_, (_, tx)) in drones.iter() {
            assert!(
                tx.send(wg_internal::controller::DroneCommand::Crash)
                    .is_ok()
            );
            // we can't fully check rx without running simulation events
        }

        for (_, (_, tx)) in clients {
            assert!(tx.send(Box::new(NodeCommand::Shutdown)).is_ok());
            // we can't fully check rx without running simulation events
        }

        for (_, (_, tx)) in servers {
            assert!(tx.send(Box::new(NodeCommand::Shutdown)).is_ok());
            // we can't fully check rx without running simulation events
        }
        running.stop_simulation();
    }
}