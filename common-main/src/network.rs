use tokio::sync::{oneshot, RwLock};
use wg_internal::network::SourceRoutingHeader;
use wg_internal::{network::NodeId, packet::Packet};
use wg_internal::packet::{FloodRequest, NodeType, PacketType};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use crate::types::{ PendingQueue, SendingMap };



pub enum NetworkError {
    TopologyError,
    PathNotFound,
    NodeNotFound,
}


pub struct Node {
    id: NodeId,
    node_type: NodeType,
    adjacents: Vec<NodeId>
}

impl Node {
    pub fn new(id: NodeId, node_type: NodeType, adjacents: Vec<NodeId>) -> Self {
        Self { id, node_type, adjacents }
    }

    pub fn get_id(&self) -> NodeId {
        self.id
    }

    pub fn get_node_type(&self) -> NodeType {
        self.node_type.clone()
    }

    pub fn get_adjacents(&self) -> &Vec<NodeId> {
        &self.adjacents
    }

    pub fn add_adjacent(&mut self, adj: NodeId) {
        self.adjacents.push(adj);
    }

    pub fn remove_adjacent(&mut self, adj: NodeId) {
        let index_to_remove = self.adjacents.iter().position(|i| *i == adj).expect(&format!("Node with id {} not found in {} adjacents", adj, self.id));
        let _ = self.adjacents.remove(index_to_remove);
    }
}

impl std::fmt::Debug for Node{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[ id: {:?}, adjacents: {:?} ]", self.id, self.adjacents)
    }
}

pub struct Network {
    pub nodes: Vec<Node>
}

impl Network {
    pub fn new(root: Node) -> Self {
        let mut nodes = vec![];
        nodes.push(root);
        Self { nodes }
    }

    pub fn add_node(&mut self, new_node: Node) -> Result<(), NetworkError> {
        for adj in new_node.get_adjacents() {
            if let Some(node) = self.nodes.iter_mut().find(|n| n.get_id() == *adj) {
                match (new_node.get_node_type(), node.get_node_type()) {
                    (_, NodeType::Drone) => {
                        node.add_adjacent(*adj);
                    }
                    _ => {
                        return Err(NetworkError::TopologyError);
                    }
                }
            }
        }

        self.nodes.push(new_node);
        Ok(())
    }

    pub fn remove_node(&mut self, node_id: NodeId) -> Result<(), NetworkError> {
        if let Some(_) = self.nodes.iter().find(|n| n.get_id() == node_id) {
            for n in self.nodes.iter_mut() {
                if n.get_adjacents().contains(&node_id){
                    n.remove_adjacent(node_id);
                }
            }
            let index_to_remove = self.nodes.iter().position(|n| n.get_id() == node_id).expect(&format!("Node {} is not a node of the network", node_id));
            let _ = self.nodes.remove(index_to_remove);
            return Ok(());
        } else {
            return Err(NetworkError::NodeNotFound);
        }
    }

    pub fn update_node(&mut self, node_id: NodeId, adjacents: Vec<NodeId>) -> Result<(), NetworkError> {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.get_id() == node_id) {
            for adj in adjacents {
                if !node.get_adjacents().contains(&adj) {
                    node.add_adjacent(adj);
                }
            }

            // teoretically no need to update neighbors of the node since they should update
            // automatically by the protocol

            return Ok(());
        } else {
            return Err(NetworkError::NodeNotFound);
        }
    }

    pub fn find_path(&self, destination: NodeId) -> Result<Vec<NodeId>, NetworkError> {
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
                return Ok(path);
            }

            if let Some(node) = self.nodes.iter().find(|n| n.get_id() == current) {
                for neighbor in node.get_adjacents().iter() {
                    if !visited.contains(neighbor) {
                        visited.insert(*neighbor);
                        parent_map.insert(neighbor, current);
                        queue.push_back(*neighbor);
                    }
                }
            }
        }
        Err(NetworkError::PathNotFound)
    }

    pub fn discover(topology: Arc<RwLock<Self>>, client_id: NodeId, neighbors: SendingMap, flood_id: u64, session_id: u64, queue: PendingQueue) -> tokio::task::JoinHandle<()> {
        tokio::task::spawn(async move {
            for (id, sender) in neighbors.write().await.iter_mut() {
                match sender.send(
                    Packet::new_flood_request(
                        SourceRoutingHeader::empty_route(),
                        session_id,
                        FloodRequest::new(flood_id, client_id )
                    )
                ) {
                    Ok(_) => {
                        let (tx, rx) = oneshot::channel();

                        {
                            let mut lock = queue.lock().await;
                            lock.insert(session_id, tx);
                        }

                        let packet = rx.await.expect("Cannot receive from oneshot channel");

                        {
                            let mut lock = queue.lock().await;
                            lock.remove(&session_id);
                        }

                        match packet {
                            PacketType::FloodResponse(flood_response) => {
                                let neighbors = flood_response.path_trace
                                    .iter()
                                    .enumerate()
                                    .map(|(i, &(node_id, node_type))| {
                                        let mut neigh = Vec::new();

                                        if i > 0 {
                                            neigh.push(flood_response.path_trace[i - 1]);
                                        }

                                        if i + 1 < flood_response.path_trace.len() {
                                            neigh.push(flood_response.path_trace[i + 1]);
                                        }

                                        ((node_id, node_type), neigh)
                                    })
                                .collect::<Vec<_>>();

                                for (node, neighbors) in neighbors.iter() {
                                    let (id, node_type) = node;
                                    let neigh_ids: Vec<u8> = neighbors.iter().map(|(n_id, _)| *n_id).collect();
                                    match topology.write().await.update_node(*id, neigh_ids.clone()) {
                                        Ok(_) => {},
                                        Err(_) => {
                                            let _ = topology.write().await.add_node(Node::new(*id, *node_type, neigh_ids));
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(_) => eprintln!("Failed to send message to {}", id),
                }
            }
        })
    }
}
