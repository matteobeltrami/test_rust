use std::collections::{HashMap, HashSet};

use crate::utils::NodeType;
use toml;
use wg_internal::config::Config;
use wg_internal::network::NodeId;

use crate::errors::ConfigError;

pub trait Parse {
    fn parse_config(path: &str) -> Result<Config, ConfigError>;
}

impl Parse for Config {
    fn parse_config(path: &str) -> Result<Config, ConfigError> {
        if path.is_empty() {
            return Err(ConfigError::EmptyPath);
        }
        let _path = std::path::Path::new(path);
        if !_path.exists() {
            return Err(ConfigError::ConfigNotFound(path.to_string()));
        }

        let content =
            std::fs::read_to_string(path).map_err(|e| ConfigError::ParseError(e.to_string()))?;

        let config: Config =
            toml::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))?;

        Ok(config)
    }
}

pub trait Validate {
    fn validate_config(&self) -> Result<(), ConfigError>;
}

impl Validate for Config {
    fn validate_config(&self) -> Result<(), ConfigError> {
        let drone_ids: HashSet<NodeId> = self.drone.iter().map(|d| d.id).collect();
        let client_ids: HashSet<NodeId> = self.client.iter().map(|c| c.id).collect();
        let server_ids: HashSet<NodeId> = self.server.iter().map(|s| s.id).collect();

        let all_ids: HashSet<NodeId> = drone_ids
            .union(&client_ids)
            .cloned()
            .collect::<HashSet<_>>()
            .union(&server_ids)
            .cloned()
            .collect();

        if all_ids.len() != (drone_ids.len() + client_ids.len() + server_ids.len()) {
            return Err(ConfigError::DuplicateNodeId(
                "Duplicate IDs found across drones, clients, and servers".to_string(),
            ));
        }

        if drone_ids.len() != self.drone.len() {
            return Err(ConfigError::InvalidConfig(
                "Duplicate drone IDs found in configuration".to_string(),
            ));
        }

        if client_ids.len() != self.client.len() {
            return Err(ConfigError::DuplicateNodeId(
                "Duplicate client IDs found in configuration".to_string(),
            ));
        }

        if server_ids.len() != self.server.len() {
            return Err(ConfigError::DuplicateNodeId(
                "Duplicate server IDs found in configuration".to_string(),
            ));
        }

        if let Some(_drone) = self.drone.iter().find(|d| d.pdr < 0.0 || d.pdr > 1.0) {
            return Err(ConfigError::InvalidPdrValue);
        }

        if let Some(drone) = self
            .drone
            .iter()
            .find(|d| d.connected_node_ids.contains(&d.id))
        {
            return Err(ConfigError::InvalidNodeConnection(format!(
                "Drone {} cannot be connected to itself",
                drone.id
            )));
        }

        if let Some(client) = self
            .client
            .iter()
            .find(|c| c.connected_drone_ids.contains(&c.id))
        {
            return Err(ConfigError::InvalidNodeConnection(format!(
                "Client {} cannot be connected to itself",
                client.id
            )));
        }

        if let Some(server) = self
            .server
            .iter()
            .find(|s| s.connected_drone_ids.contains(&s.id))
        {
            return Err(ConfigError::InvalidNodeConnection(format!(
                "Server {} cannot be connected to itself",
                server.id
            )));
        }

        if let Some(client) = self.client.iter().find(|c| {
            c.connected_drone_ids
                .iter()
                .any(|id| client_ids.contains(id))
                || c.connected_drone_ids
                    .iter()
                    .any(|id| server_ids.contains(id))
        }) {
            return Err(ConfigError::InvalidNodeConnection(format!(
                "Client {} cannot have other clients  or servers in its connected_drone_ids",
                client.id
            )));
        }

        if let Some(client) = self
            .client
            .iter()
            .find(|c| c.connected_drone_ids.len() < 1 || c.connected_drone_ids.len() > 2)
        {
            return Err(ConfigError::InvalidConfig(format!(
                "Client {} must have exactly one or two connected drones",
                client.id
            )));
        }

        if let Some(server) = self.server.iter().find(|s| {
            s.connected_drone_ids
                .iter()
                .any(|id| client_ids.contains(id))
                || s.connected_drone_ids
                    .iter()
                    .any(|id| server_ids.contains(id))
        }) {
            return Err(ConfigError::InvalidConfig(format!(
                "Server {} cannot have other clients or servers in its connected_drone_ids",
                server.id
            )));
        }

        if let Some(server) = self.server.iter().find(|s| s.connected_drone_ids.len() < 2) {
            return Err(ConfigError::InvalidConfig(format!(
                "Server {} must have at least two connected drones",
                server.id
            )));
        }

        let node_map: HashMap<NodeId, NodeType> = self
            .drone
            .iter()
            .map(|d| (d.id, NodeType::Drone(d)))
            .chain(self.client.iter().map(|c| (c.id, NodeType::Client(c))))
            .chain(self.server.iter().map(|s| (s.id, NodeType::Server(s))))
            .collect();

        for node in node_map.values() {
            for connected_id in node.connected_node_ids() {
                if let Some(connected_node) = node_map.get(connected_id) {
                    if !connected_node.connected_node_ids().contains(&node.id()) {
                        return Err(ConfigError::UnidirectedConnection);
                    }
                }
            }
        }

        Ok(())
    }
}
