use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use crossbeam::channel::{Receiver, Sender};
use wg_internal::{
    network::{NodeId, SourceRoutingHeader},
    packet::{Packet, PacketType, Fragment, Ack, FloodRequest, FloodResponse, NodeType},
    controller::{DroneEvent, DroneCommand},
};
use log::{info, warn, error, debug};
use crate::types::{FragmentAssembler, MessageFragmenter, PendingQueue, SendingMap};
use crate::network::{Network, Node, NetworkError};

/// Base functionality shared by all server types
#[derive(Debug)]
pub struct ServerBase {
    pub id: NodeId,
    pub server_type: NodeType,
    pub neighbors: SendingMap,
    pub topology: Arc<RwLock<Network>>,
    pub assembler: Arc<Mutex<FragmentAssembler>>,
    pub pending_responses: PendingQueue,
    pub session_counter: Arc<Mutex<u64>>,
    pub flood_counter: Arc<Mutex<u64>>,
    // Communication channels
    pub packet_receiver: Option<Receiver<Packet>>,
    pub controller_sender: Option<Sender<DroneEvent>>,
    pub controller_receiver: Option<Receiver<DroneCommand>>,
}

impl ServerBase {
    pub fn new(id: NodeId) -> Self {
        // Initialize with self as root node of topology
        let root_node = Node::new(id, NodeType::Server, vec![]);
        let topology = Network::new(root_node);
        
        Self {
            id,
            server_type: NodeType::Server,
            neighbors: Arc::new(RwLock::new(HashMap::new())),
            topology: Arc::new(RwLock::new(topology)),
            assembler: Arc::new(Mutex::new(FragmentAssembler::new())),
            pending_responses: Arc::new(Mutex::new(HashMap::new())),
            session_counter: Arc::new(Mutex::new(1)),
            flood_counter: Arc::new(Mutex::new(1)),
            packet_receiver: None,
            controller_sender: None,
            controller_receiver: None,
        }
    }

    /// Set communication channels (called by network initializer)
    pub fn set_channels(
        &mut self,
        packet_receiver: Receiver<Packet>,
        controller_sender: Sender<DroneEvent>,
        controller_receiver: Receiver<DroneCommand>,
    ) {
        self.packet_receiver = Some(packet_receiver);
        self.controller_sender = Some(controller_sender);
        self.controller_receiver = Some(controller_receiver);
    }

    /// Add a drone neighbor
    pub async fn add_neighbor(&self, drone_id: NodeId, sender: Sender<Packet>) {
        let mut neighbors = self.neighbors.write().await;
        neighbors.insert(drone_id, sender);
        info!("Server {} added neighbor drone {}", self.id, drone_id);
    }

    /// Remove a drone neighbor
    pub async fn remove_neighbor(&self, drone_id: NodeId) {
        let mut neighbors = self.neighbors.write().await;
        neighbors.remove(&drone_id);
        info!("Server {} removed neighbor drone {}", self.id, drone_id);
    }

    /// Generate a new session ID
    pub async fn new_session_id(&self) -> u64 {
        let mut counter = self.session_counter.lock().await;
        let session_id = *counter;
        *counter += 1;
        session_id
    }

    /// Generate a new flood ID
    pub async fn new_flood_id(&self) -> u64 {
        let mut counter = self.flood_counter.lock().await;
        let flood_id = *counter;
        *counter += 1;
        flood_id
    }

    /// Discover network topology using flood protocol
    pub async fn discover_network(&self) -> Result<(), Box<dyn std::error::Error>> {
        let flood_id = self.new_flood_id().await;
        let session_id = self.new_session_id().await;
        
        info!("Server {} starting network discovery with flood_id: {}", self.id, flood_id);
        
        // Start network discovery
        let _discovery_handle = crate::network::Network::discover(
            self.topology.clone(),
            self.id,
            self.neighbors.clone(),
            flood_id,
            session_id,
            self.pending_responses.clone(),
        );

        Ok(())
    }

    /// Send a message to a destination using source routing
    pub async fn send_message(
        &self,
        destination: NodeId,
        message_data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Find path to destination
        let path = {
            let topology = self.topology.read().await;
            topology.find_path(destination).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?
        };

        info!("Server {} sending message to {} via path: {:?}", self.id, destination, path);

        // Fragment the message if needed
        let fragments = MessageFragmenter::fragment_message(message_data);
        let session_id = self.new_session_id().await;

        // Send each fragment
        for (fragment_index, total_fragments, length, data) in fragments {
            let fragment = Fragment {
                fragment_index,
                total_n_fragments: total_fragments,
                length,
                data,
            };

            let routing_header = SourceRoutingHeader {
                hop_index: 1,
                hops: path.clone(),
            };

            let packet = Packet {
                pack_type: PacketType::MsgFragment(fragment),
                routing_header,
                session_id,
            };

            // Send to first hop (should be a drone neighbor)
            if let Some(first_hop) = path.get(1) {
                let neighbors = self.neighbors.read().await;
                if let Some(sender) = neighbors.get(first_hop) {
                    if let Err(e) = sender.send(packet.clone()) {
                        error!("Failed to send fragment to {}: {}", first_hop, e);
                    } else {
                        debug!("Sent fragment {} of {} to {}", fragment_index + 1, total_fragments, destination);
                    }
                } else {
                    error!("No sender found for neighbor {}", first_hop);
                }
            }
        }

        Ok(())
    }

    /// Handle incoming packet
    pub async fn handle_packet(&self, packet: Packet) -> Option<Vec<u8>> {
        match packet.pack_type {
            PacketType::MsgFragment(fragment) => {
                self.handle_fragment(packet.routing_header, packet.session_id, fragment).await
            },
            PacketType::FloodRequest(flood_request) => {
                self.handle_flood_request(flood_request, packet.session_id).await;
                None
            },
            PacketType::FloodResponse(flood_response) => {
                self.handle_flood_response(flood_response, packet.session_id).await;
                None
            },
            PacketType::Ack(ack) => {
                debug!("Received ACK for fragment {}", ack.fragment_index);
                None
            },
            PacketType::Nack(nack) => {
                warn!("Received NACK for fragment {}: {:?}", nack.fragment_index, nack.nack_type);
                None
            },
        }
    }

    /// Handle incoming message fragment
    async fn handle_fragment(
        &self,
        routing_header: SourceRoutingHeader,
        session_id: u64,
        fragment: Fragment,
    ) -> Option<Vec<u8>> {
        let src_id = routing_header.hops.first().copied().unwrap_or(0);
        
        debug!(
            "Received fragment {} of {} from {} (session: {})",
            fragment.fragment_index + 1,
            fragment.total_n_fragments,
            src_id,
            session_id
        );

        // Send ACK back to sender
        self.send_ack(src_id, fragment.fragment_index, session_id).await;

        // Try to assemble the complete message
        let complete_message = {
            let mut assembler = self.assembler.lock().await;
            assembler.add_fragment(
                session_id,
                fragment.fragment_index,
                fragment.total_n_fragments,
                fragment.length,
                &fragment.data,
            )
        };

        if let Some(message_data) = complete_message {
            info!(
                "Completed message reassembly from {} (session: {}, {} bytes)",
                src_id,
                session_id,
                message_data.len()
            );
            return Some(message_data);
        }

        None
    }

    /// Send ACK back to sender
    async fn send_ack(&self, destination: NodeId, fragment_index: u64, session_id: u64) {
        // Find path back to sender
        if let Ok(path) = {
            let topology = self.topology.read().await;
            topology.find_path(destination)
        } {
            let ack = Ack { fragment_index };
            let routing_header = SourceRoutingHeader {
                hop_index: 1,
                hops: path,
            };

            let packet = Packet {
                pack_type: PacketType::Ack(ack),
                routing_header,
                session_id,
            };

            // Send ACK
            if let Some(first_hop) = packet.routing_header.hops.get(1) {
                let neighbors = self.neighbors.read().await;
                if let Some(sender) = neighbors.get(first_hop) {
                    if let Err(e) = sender.send(packet) {
                        error!("Failed to send ACK to {}: {}", destination, e);
                    }
                }
            }
        }
    }

    /// Handle flood request for network discovery
    async fn handle_flood_request(&self, flood_request: FloodRequest, session_id: u64) {
        debug!("Handling flood request from {}", flood_request.initiator_id);
        
        // Since we're a server (leaf node), we just send back a flood response
        let flood_response = FloodResponse {
            flood_id: flood_request.flood_id,
            path_trace: flood_request.path_trace.clone(),
        };

        // Create route back to initiator by reversing the path_trace
        let mut route: Vec<NodeId> = flood_request.path_trace
            .iter()
            .map(|(id, _)| *id)
            .collect();
        route.reverse();
        route.push(flood_request.initiator_id);

        let routing_header = SourceRoutingHeader {
            hop_index: 1,
            hops: route,
        };

        let packet = Packet {
            pack_type: PacketType::FloodResponse(flood_response),
            routing_header,
            session_id,
        };

        // Send response back
        if let Some(first_hop) = packet.routing_header.hops.get(1) {
            let neighbors = self.neighbors.read().await;
            if let Some(sender) = neighbors.get(first_hop) {
                if let Err(e) = sender.send(packet) {
                    error!("Failed to send flood response: {}", e);
                }
            }
        }
    }

    /// Handle flood response from network discovery
    async fn handle_flood_response(&self, flood_response: FloodResponse, session_id: u64) {
        debug!("Received flood response for flood_id: {}", flood_response.flood_id);
        
        // Check if we have a pending response for this session
        if let Some(sender) = {
            let mut pending = self.pending_responses.lock().await;
            pending.remove(&session_id)
        } {
            if let Err(_) = sender.send(PacketType::FloodResponse(flood_response)) {
                warn!("Failed to send flood response to pending channel");
            }
        }
    }

    /// Serialize a message to bytes using bincode
    pub fn serialize_message<T: serde::Serialize>(message: &T) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(message)
    }

    /// Deserialize bytes to a message using bincode
    pub fn deserialize_message<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T, bincode::Error> {
        bincode::deserialize(data)
    }
}