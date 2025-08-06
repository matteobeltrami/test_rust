use wg_internal::config::{Client, Drone, Server};
use wg_internal::network::NodeId;

pub enum NodeType<'a> {
    Drone(&'a Drone),
    Client(&'a Client),
    Server(&'a Server),
}

impl<'a> NodeType<'a> {
    pub fn id(&'a self) -> NodeId {
        match self {
            NodeType::Drone(drone) => drone.id,
            NodeType::Client(client) => client.id,
            NodeType::Server(server) => server.id,
        }
    }

    pub fn connected_node_ids(&'a self) -> &'a Vec<NodeId> {
        match self {
            NodeType::Drone(drone) => &drone.connected_node_ids,
            NodeType::Client(client) => &client.connected_drone_ids,
            NodeType::Server(server) => &server.connected_drone_ids,
        }
    }
}
