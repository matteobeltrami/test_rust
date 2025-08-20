use crossbeam_channel::SendError;
use wg_internal::network::NodeId;
use wg_internal::packet::NodeType;
use std::{collections::{HashMap, HashSet, VecDeque}, fmt::Display};

#[derive(Debug)]
pub enum NetworkError {
    TopologyError,
    PathNotFound(u8),
    NodeNotFound(u8),
    NodeIsNotANeighbor(u8),
    SendError(String),
    ControllerDisconnected,
    NoDestination,
    NoNeighborAssigned
}

impl Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TopologyError => write!(f, "Topology error"),
            Self::PathNotFound(id) => write!(f, "Path not found for node {id}"),
            Self::NodeNotFound(id) => write!(f, "Node {id} not found"),
            Self::NodeIsNotANeighbor(id) => write!(f, "Node {id} is not a neighbor"),
            Self::SendError(msg) => write!(f, "Send error: {msg}"),
            Self::ControllerDisconnected => write!(f, "Controller disconnected"),
            Self::NoDestination => write!(f, "Packet has no destination specified"),
            Self::NoNeighborAssigned => write!(f, "No neighbor assigned"),
        }
    }
}

impl std::error::Error for NetworkError {}


impl<T: Send + std::fmt::Debug> From<SendError<T>> for NetworkError {
    fn from(value: SendError<T>) -> Self {
        NetworkError::SendError(format!("{value:?}"))
    }
}


#[derive(Clone)]
pub(crate) struct Node {
    pub id: NodeId,
    kind: NodeType,
    adjacents: Vec<NodeId>
}


impl Node {
    #[must_use]
    pub fn new(id: NodeId, kind: NodeType, adjacents: Vec<NodeId>) -> Self {
        Self { id, kind, adjacents }
    }

    pub fn get_id(&self) -> NodeId {
        self.id
    }

    pub fn get_node_type(&self) -> NodeType {
        self.kind
    }

    #[must_use]
    pub fn get_adjacents(&self) -> &Vec<NodeId> {
        &self.adjacents
    }

    pub fn add_adjacent(&mut self, adj: NodeId) {
        self.adjacents.push(adj);
    }

    pub fn remove_adjacent(&mut self, adj: NodeId) {
        if let Some(index_to_remove) = self.adjacents.iter().position(|i| *i == adj) {
            let _ = self.adjacents.remove(index_to_remove);
        }
    }
}

impl std::fmt::Debug for Node{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[ id: {:?}, adjacents: {:?} ]", self.id, self.adjacents)
    }
}

#[derive(Debug, Clone)]
pub struct Network {
    pub(crate) nodes: Vec<Node>
}

impl Network {
    #[must_use]
    pub(crate) fn new(root: Node) -> Self {
        let nodes = vec![root];
        Self { nodes }
    }


    pub(crate) fn add_node(&mut self, new_node: Node) {
        for adj in new_node.get_adjacents() {
            if let Some(node) = self.nodes.iter_mut().find(|n| n.id == *adj) {
                match (new_node.get_node_type(), node.get_node_type()) {
                    (_, NodeType::Drone) | (NodeType::Drone, _) => {
                        node.add_adjacent(*adj);
                    }
                    _ => {}
                }
            }
        }

        self.nodes.push(new_node);
    }

    pub(crate) fn remove_node(&mut self, node_id: NodeId) {
        for n in &mut self.nodes{
            if n.get_adjacents().contains(&node_id){
                n.remove_adjacent(node_id);
            }
        }
        if let Some(index_to_remove) = self.nodes.iter().position(|n| n.id == node_id) {
            let _ = self.nodes.remove(index_to_remove);
        }
    }

    /// Updates the node's adjacents with the provided list.
    /// # Errors
    /// If the node is not found, returns an error.
    pub(crate) fn update_node(&mut self, node_id: NodeId, adjacents: Vec<NodeId>) -> Result<(), NetworkError> {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            for adj in adjacents {
                if !node.get_adjacents().contains(&adj) {
                    node.add_adjacent(adj);
                }
            }

            // teoretically no need to update neighbors of the node since they should update
            // automatically by the protocol

            return Ok(());
        }
        Err(NetworkError::NodeNotFound(node_id))
    }

    pub(crate) fn change_node_type(&mut self, id: NodeId, new_type: NodeType) {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.get_id() == id) {
                node.kind = new_type;
        }
    }


    /// BFS to find path to destination
    #[must_use]
    pub(crate) fn find_path(&self, destination: NodeId) -> Option<Vec<NodeId>> {
        let start = self.nodes[0].id;
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut parent_map = HashMap::new();

        queue.push_back(start);
        visited.insert(start);

        while let Some(current) = queue.pop_front() {
            if current == destination {
                let mut path = vec![destination];
                let mut current = destination;
                while let Some(&parent) = parent_map.get(&current) {
                    path.push(parent);
                    current = parent;
                }
                path.reverse();
                return Some(path);
            }

            if let Some(node) = self.nodes.iter().find(|n| n.id == current) {
                for neighbor in node.get_adjacents() {
                    if !visited.contains(neighbor) {
                        visited.insert(*neighbor);
                        parent_map.insert(neighbor, current);
                        queue.push_back(*neighbor);
                    }
                }
            }
        }
        None
    }

    #[must_use]
    pub fn get_servers(&self) -> Option<Vec<NodeId>> {
        let servers = self.nodes.iter().filter_map(|n| {
            if n.get_node_type() == NodeType::Server {
                Some(n.get_id())
            }
            else {
                None
            }
        }).collect::<Vec<_>>();

        if servers.is_empty() {
            None
        }else {
            Some(servers)
        }
    }


    #[must_use]
    pub fn get_clients(&self) -> Option<Vec<NodeId>> {
        let clients = self.nodes.iter().filter_map(|n| {
            if n.get_node_type() == NodeType::Client {
                Some(n.get_id())
            }
            else {
                None
            }
        }).collect::<Vec<_>>();

        if clients.is_empty() {
            None
        }else {
            Some(clients)
        }

    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Tests adding a node to the network
    fn test_add_node() {
        let root = Node::new(1, NodeType::Client, vec![2, 3]);
        let mut network = Network::new(root);

        let new_node = Node::new(2, NodeType::Client, vec![1]);
        network.add_node(new_node);

        assert_eq!(network.nodes.len(), 2);
        assert!(network.nodes.iter().any(|n| n.id == 2));
    }

    #[test]
    /// Tests removing a node from the network
    fn test_remove_node() {
        let root = Node::new(1, NodeType::Client, vec![2, 3]);
        let mut network = Network::new(root);

        let new_node = Node::new(2, NodeType::Client, vec![1]);
        network.add_node(new_node);

        network.remove_node(2);

        assert_eq!(network.nodes.len(), 1);
        assert!(!network.nodes.iter().any(|n| n.id == 2));
    }

    #[test]
    /// Tests updating a node
    fn test_update_node() {
        let root = Node::new(1, NodeType::Client, vec![2]);
        let mut network = Network::new(root);

        let new_node = Node::new(2, NodeType::Client, vec![1]);
        network.add_node(new_node);

        network.update_node(1, vec![3]).unwrap();

        assert!(network.nodes[0].get_adjacents().contains(&3));
    }

    #[test]
    /// Tests changing the `NodeType` to a node
    fn test_change_node_type() {
        let root = Node::new(1, NodeType::Client, vec![2]);
        let mut network = Network::new(root);

        network.change_node_type(1, NodeType::Drone);

        assert_eq!(network.nodes[0].get_node_type(), NodeType::Drone);
    }

    #[test]
    /// Tests the `find_path` method
    fn test_find_path() {
        let root = Node::new(1, NodeType::Client, vec![2, 3]);
        let mut network = Network::new(root);

        let node2 = Node::new(2, NodeType::Client, vec![1, 4]);
        let node3 = Node::new(3, NodeType::Client, vec![1]);
        let node4 = Node::new(4, NodeType::Client, vec![2]);

        network.add_node(node2);
        network.add_node(node3);
        network.add_node(node4);

        let path = network.find_path(4).unwrap();

        assert_eq!(path, vec![1, 2, 4]);
    }

    #[test]
    /// Tests the `find_path` method with non-existing path
    fn test_find_path_not_found() {
        let root = Node::new(1, NodeType::Client, vec![2]);
        let mut network = Network::new(root);

        let node2 = Node::new(2, NodeType::Client, vec![1]);
        network.add_node(node2);

        let result = network.find_path(3);

        assert!(result.is_none());
    }

    #[test]
    /// Tests `find_path` method in a complex network
    fn test_complex_network_topology() {
        let root = Node::new(1, NodeType::Client, vec![2, 3]);
        let mut network = Network::new(root);

        network.add_node(Node::new(2, NodeType::Drone, vec![1, 4, 5]));
        network.add_node(Node::new(3, NodeType::Drone, vec![1, 6]));
        network.add_node(Node::new(4, NodeType::Drone, vec![2, 7]));
        network.add_node(Node::new(5, NodeType::Server, vec![2]));
        network.add_node(Node::new(6, NodeType::Server, vec![3]));
        network.add_node(Node::new(7, NodeType::Client, vec![4]));

        let path_to_server5 = network.find_path(5);
        assert!(path_to_server5.is_some());
        let path = path_to_server5.unwrap();
        assert_eq!(path[0], 1);
        assert_eq!(path[path.len() - 1], 5);
        let path_to_client7 = network.find_path(7);
        assert!(path_to_client7.is_some());
    }

    #[test]
    /// Tests `find_path` method in a partitioned network
    fn test_network_partition_handling() {
        let root = Node::new(1, NodeType::Client, vec![2]);
        let mut network = Network::new(root);

        network.add_node(Node::new(2, NodeType::Drone, vec![1]));
        network.add_node(Node::new(3, NodeType::Server, vec![4])); // Isolated
        network.add_node(Node::new(4, NodeType::Drone, vec![3])); // Isolated

        let path_to_2 = network.find_path(2);
        assert!(path_to_2.is_some());

        let path_to_3 = network.find_path(3);
        assert!(path_to_3.is_none());
    }

    #[test]
    /// Tests `get_servers` and `get_clients` method in a network
    fn test_node_type_filtering() {
        let root = Node::new(1, NodeType::Client, vec![]);
        let mut network = Network::new(root);

        network.add_node(Node::new(2, NodeType::Server, vec![]));
        network.add_node(Node::new(3, NodeType::Server, vec![]));
        network.add_node(Node::new(4, NodeType::Client, vec![]));
        network.add_node(Node::new(5, NodeType::Drone, vec![]));

        let servers = network.get_servers().unwrap();
        assert_eq!(servers.len(), 2);
        assert!(servers.contains(&2));
        assert!(servers.contains(&3));

        let clients = network.get_clients().unwrap();
        assert_eq!(clients.len(), 2);
        assert!(clients.contains(&1));
        assert!(clients.contains(&4));
    }

    #[test]
    fn test_dynamic_network_updates() {
        let root = Node::new(1, NodeType::Client, vec![2]);
        let mut network = Network::new(root);

        network.add_node(Node::new(2, NodeType::Drone, vec![1, 3]));
        network.add_node(Node::new(3, NodeType::Server, vec![2]));

        network.update_node(1, vec![4]).unwrap();
        network.add_node(Node::new(4, NodeType::Drone, vec![1]));

        let path = network.find_path(3);
        assert!(path.is_some());

        network.remove_node(2);
        let path_after_removal = network.find_path(3);
        assert!(path_after_removal.is_none());
    }
}
