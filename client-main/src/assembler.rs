use std::collections::HashMap;
use wg_internal::packet::{Packet, Fragment, Ack, Nack, NackType, PacketType};
use wg_internal::network::{SourceRoutingHeader, NodeId};
use common::types::{FragmentAssembler, MessageFragmenter, ClientError};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};
use crossbeam::channel::Sender;

/// Assembler handles fragmentation, serialization, and reassembly of messages
pub struct Assembler {
    id: NodeId,
    session_counter: Arc<Mutex<u64>>,
    fragment_assembler: Arc<Mutex<FragmentAssembler>>,
    neighbors: Arc<Mutex<HashMap<NodeId, Sender<Packet>>>>,
    pending_responses: Arc<Mutex<HashMap<u64, oneshot::Sender<Vec<u8>>>>>,
}

impl Assembler {
    pub fn new(id: NodeId, neighbors: HashMap<NodeId, Sender<Packet>>) -> Self {
        Self {
            id,
            session_counter: Arc::new(Mutex::new(0)),
            fragment_assembler: Arc::new(Mutex::new(FragmentAssembler::new())),
            neighbors: Arc::new(Mutex::new(neighbors)),
            pending_responses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Send a high-level message to a destination, handling fragmentation and routing
    pub async fn send_message<T: Serialize>(
        &self,
        message: &T,
        destination: NodeId,
        route: Vec<NodeId>,
    ) -> Result<Vec<u8>, ClientError> {
        // Serialize the message
        let serialized = self.serialize_message(message)?;
        println!("📤 Sending message to {} via route {:?} ({} bytes)", 
                 destination, route, serialized.len());
        
        // Get next session ID
        let session_id = {
            let mut counter = self.session_counter.lock().await;
            *counter += 1;
            *counter
        };

        // Fragment the message
        let fragments = MessageFragmenter::fragment_message(&serialized);
        println!("📦 Message fragmented into {} fragments", fragments.len());

        // Create oneshot channel for response
        let (response_tx, response_rx) = oneshot::channel();
        {
            let mut pending = self.pending_responses.lock().await;
            pending.insert(session_id, response_tx);
        }

        // Send all fragments
        for (fragment_index, total_fragments, length, data) in fragments {
            let fragment = Fragment {
                fragment_index,
                total_n_fragments: total_fragments,
                length,
                data,
            };

            let packet = Packet {
                pack_type: PacketType::MsgFragment(fragment),
                routing_header: SourceRoutingHeader {
                    hop_index: 1, // Start at 1 (sender is at index 0)
                    hops: route.clone(),
                },
                session_id,
            };

            // Send packet through first hop (if route has more than just sender)
            if route.len() > 1 {
                let first_hop = route[1];
                let neighbors = self.neighbors.lock().await;
                if let Some(sender) = neighbors.get(&first_hop) {
                    sender.send(packet).map_err(|e| 
                        ClientError::NetworkError(format!("Failed to send fragment {} to {}: {:?}", 
                                                          fragment_index, first_hop, e)))?;
                    println!("✅ Sent fragment {} to {}", fragment_index, first_hop);
                } else {
                    return Err(ClientError::NetworkError(format!("No connection to next hop {}", first_hop)));
                }
            } else {
                return Err(ClientError::NetworkError("Invalid route: must have at least sender and one hop".to_string()));
            }
        }

        // Wait for response
        match tokio::time::timeout(std::time::Duration::from_secs(30), response_rx).await {
            Ok(Ok(response)) => {
                println!("📥 Received response ({} bytes)", response.len());
                Ok(response)
            },
            Ok(Err(_)) => Err(ClientError::NetworkError("Response channel closed".to_string())),
            Err(_) => {
                eprintln!("⏱️ Timeout waiting for response to session {}", session_id);
                // Cleanup pending response
                let mut pending = self.pending_responses.lock().await;
                pending.remove(&session_id);
                Err(ClientError::TimeoutError)
            },
        }
    }

    /// Handle incoming packets (fragments, acks, nacks)
    pub async fn handle_packet(&self, packet: Packet) -> Result<(), ClientError> {
        let routing_header = packet.routing_header.clone();
        match packet.pack_type {
            PacketType::MsgFragment(fragment) => {
                self.handle_fragment(packet.session_id, fragment, routing_header).await
            },
            PacketType::Ack(ack) => {
                self.handle_ack(packet.session_id, ack).await
            },
            PacketType::Nack(nack) => {
                self.handle_nack(packet.session_id, nack).await
            },
            _ => Ok(()), // Other packet types handled elsewhere
        }
    }

    async fn handle_fragment(&self, session_id: u64, fragment: Fragment, source_route: SourceRoutingHeader) -> Result<(), ClientError> {
        println!("📦 Handling fragment {} of {} for session {}", 
                 fragment.fragment_index, fragment.total_n_fragments, session_id);
        
        let mut assembler = self.fragment_assembler.lock().await;
        
        if let Some(complete_data) = assembler.add_fragment(
            session_id,
            fragment.fragment_index,
            fragment.total_n_fragments,
            fragment.length,
            &fragment.data,
        ) {
            println!("✅ Complete message reassembled for session {} ({} bytes)", session_id, complete_data.len());
            
            // Send Ack for completed message
            let ack = Ack {
                fragment_index: fragment.fragment_index,
            };
            
            // Create reverse route for Ack
            let mut reverse_route = source_route.hops.clone();
            reverse_route.reverse();
            
            let ack_packet = Packet {
                pack_type: PacketType::Ack(ack),
                routing_header: SourceRoutingHeader {
                    hop_index: 1, // Start routing back
                    hops: reverse_route,
                },
                session_id,
            };

            // Send Ack back to sender
            self.send_packet_to_first_hop(ack_packet).await?;

            // Complete message received, send to pending response handler
            let mut pending = self.pending_responses.lock().await;
            if let Some(response_sender) = pending.remove(&session_id) {
                let _ = response_sender.send(complete_data);
            }
        } else {
            println!("📦 Fragment {} received, waiting for more fragments", fragment.fragment_index);
            
            // Send Ack for individual fragment
            let ack = Ack {
                fragment_index: fragment.fragment_index,
            };
            
            // Create reverse route for Ack
            let mut reverse_route = source_route.hops.clone();
            reverse_route.reverse();
            
            let ack_packet = Packet {
                pack_type: PacketType::Ack(ack),
                routing_header: SourceRoutingHeader {
                    hop_index: 1,
                    hops: reverse_route,
                },
                session_id,
            };
            
            // Send Ack back to sender
            self.send_packet_to_first_hop(ack_packet).await?;
        }

        Ok(())
    }

    /// Send a packet to the first hop in its route
    async fn send_packet_to_first_hop(&self, packet: Packet) -> Result<(), ClientError> {
        if packet.routing_header.hops.len() > 1 {
            let first_hop = packet.routing_header.hops[1];
            let neighbors = self.neighbors.lock().await;
            if let Some(sender) = neighbors.get(&first_hop) {
                sender.send(packet).map_err(|e| 
                    ClientError::NetworkError(format!("Failed to send packet: {:?}", e)))?;
                println!("📤 Sent packet to first hop {}", first_hop);
            } else {
                return Err(ClientError::NetworkError(format!("No connection to {}", first_hop)));
            }
        }
        Ok(())
    }

    async fn handle_ack(&self, _session_id: u64, _ack: Ack) -> Result<(), ClientError> {
        // Handle acknowledgment of sent fragments
        // This would be used for reliable transmission if implementing retransmission
        Ok(())
    }

    async fn handle_nack(&self, session_id: u64, nack: Nack) -> Result<(), ClientError> {
        // Handle negative acknowledgment
        let error_msg = match nack.nack_type {
            NackType::ErrorInRouting(node_id) => format!("Routing error at node {}", node_id),
            NackType::DestinationIsDrone => "Destination is drone".to_string(),
            NackType::Dropped => "Packet was dropped".to_string(),
            NackType::UnexpectedRecipient(node_id) => format!("Unexpected recipient: {}", node_id),
        };

        // Notify pending response of error
        let mut pending = self.pending_responses.lock().await;
        if let Some(response_sender) = pending.remove(&session_id) {
            let _ = response_sender.send(vec![]); // Empty response indicates error
        }

        Err(ClientError::ProtocolError(error_msg))
    }

    fn serialize_message<T: Serialize>(&self, message: &T) -> Result<Vec<u8>, ClientError> {
        bincode::serialize(message)
            .map_err(|e| ClientError::SerializationError(e.to_string()))
    }

    pub fn deserialize_message<T: for<'de> Deserialize<'de>>(&self, data: &[u8]) -> Result<T, ClientError> {
        bincode::deserialize(data)
            .map_err(|e| ClientError::SerializationError(e.to_string()))
    }

    /// Update neighbors (for simulation controller commands)
    pub async fn update_neighbors(&self, neighbors: HashMap<NodeId, Sender<Packet>>) {
        let mut current_neighbors = self.neighbors.lock().await;
        *current_neighbors = neighbors;
    }

    /// Add a new neighbor
    pub async fn add_neighbor(&self, node_id: NodeId, sender: Sender<Packet>) {
        let mut neighbors = self.neighbors.lock().await;
        neighbors.insert(node_id, sender);
    }

    /// Remove a neighbor
    pub async fn remove_neighbor(&self, node_id: NodeId) {
        let mut neighbors = self.neighbors.lock().await;
        neighbors.remove(&node_id);
    }
}