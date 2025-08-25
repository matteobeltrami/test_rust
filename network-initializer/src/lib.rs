mod errors;
pub mod network_initializer;
mod parser;
#[macro_use]
mod utils;

#[cfg(test)]
mod tests {

    type Simulation = (
        NetworkInitializer<Running>,
        HashMap<NodeId, (f32, Sender<DroneCommand>)>,
        HashMap<NodeId, (NodeType, Sender<Box<dyn Command>>)>,
        HashMap<NodeId, (NodeType, Sender<Box<dyn Command>>)>,
        Network, // HashMap<NodeId, Channel<Packet>>,
    );

    use std::collections::HashMap;

    use crate::errors::ConfigError;
    use crate::network_initializer::NetworkInitializer;
    use crate::network_initializer::Running;
    use crate::network_initializer::Uninitialized;
    use crate::parser::Parse;
    use crate::parser::Validate;
    use common::network::Network;
    // use crate::utils::Channel;
    use common::types::Command;
    use common::types::NodeCommand;
    use common::types::NodeType;
    use crossbeam::channel::Sender;
    use wg_internal::config::Config;
    use wg_internal::controller::DroneCommand;
    use wg_internal::network::NodeId;
    // use wg_internal::packet::Packet;

    fn gen_simulation(path: &str) -> Simulation {
        let initializer = NetworkInitializer::<Uninitialized>::new(path)
            .initialize()
            .start_simulation();
        let clients = initializer.get_clients();
        let servers = initializer.get_servers();
        let drones = initializer.get_drones();
        let network = initializer.get_network_view();
        (initializer, drones, clients, servers, network)
    }
    
    fn stop_simulation(sim: Simulation) {
        let (mut running, _drones, _clients, _servers, _network) = sim;
        running.stop_simulation();
    }

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

        assert!(!drones.is_empty(), "Drones should not be empty");
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

    #[test]
    fn test_simple_config() {
        let config_path = "./config/simple_config.toml";
        let (running_sim, drones, clients, servers, network) = gen_simulation(config_path);
        assert_eq!(drones.len(), 2, "Drones should be 2");
        assert_eq!(clients.len(), 1, "Client should be 1");
        assert_eq!(servers.len(), 1, "Server should be 1");
        assert_eq!(network.nodes.len(), 4, "Nodes should be 4");
        assert_eq!(
            clients.get(&1).unwrap().0,
            NodeType::WebBrowser,
            "Client should be a WebBrowser"
        );
        assert_eq!(
            servers.get(&4).unwrap().0,
            NodeType::TextServer,
            "Server should be a TextServer"
        );
        assert_eq!(
            network.nodes.iter().find(|n| n.id == 1).unwrap().get_adjacents(),
            &running_sim
                .config
                .client
                .iter()
                .find(|c| c.id == 1)
                .unwrap()
                .connected_drone_ids,
            "Adjacents of client 1 are not the expected"
        );
        assert_eq!(
            network.nodes.iter().find(|n| n.id == 2).unwrap().get_adjacents(),
            &running_sim.config.drone.iter().find(|c| c.id == 2).unwrap().connected_node_ids,
            "Adjacents of drone 2 are not the expected"
        );
        assert_eq!(
            network.nodes.iter().find(|n| n.id == 3).unwrap().get_adjacents(),
            &running_sim.config.drone.iter().find(|c| c.id == 3).unwrap().connected_node_ids,
            "Adjacents of drone 3 are not the expected"
        );
        assert_eq!(
            network.nodes.iter().find(|n| n.id == 4).unwrap().get_adjacents(),
            &running_sim
                .config
                .server
                .iter()
                .find(|s| s.id == 4)
                .unwrap()
                .connected_drone_ids,
            "Adjacents of server 4 are not the expected"
        );

        stop_simulation((running_sim, drones, clients, servers, network));
    }
}
