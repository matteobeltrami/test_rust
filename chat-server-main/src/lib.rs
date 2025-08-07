use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crossbeam::channel::{Receiver, Sender, select_biased};
use wg_internal::{
    network::NodeId,
    packet::Packet,
    controller::{DroneEvent, DroneCommand},
};
use common::server_base::ServerBase;
use common::types::{ClientMessage, ServerResponse, ServerType};
use log::{info, error, debug, warn};
use serde::{Deserialize, Serialize};

/// Information about a registered client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredClient {
    pub id: NodeId,
    pub name: String,
    pub registration_time: u64,
    pub last_activity: u64,
}

/// Chat message with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub from_client_id: NodeId,
    pub from_client_name: String,
    pub to_client_id: NodeId,
    pub to_client_name: String,
    pub message: String,
    pub timestamp: u64,
}

/// Communication Server implementation - handles client registration and message forwarding
pub struct CommunicationServer {
    base: ServerBase,
    registered_clients: Arc<RwLock<HashMap<NodeId, RegisteredClient>>>,
    client_names: Arc<RwLock<HashMap<String, NodeId>>>, // name -> client_id mapping
    message_history: Arc<RwLock<Vec<ChatMessage>>>,
    server_start_time: u64,
}

impl CommunicationServer {
    /// Create a new Communication Server
    pub fn new(id: NodeId) -> Self {
        Self {
            base: ServerBase::new(id),
            registered_clients: Arc::new(RwLock::new(HashMap::new())),
            client_names: Arc::new(RwLock::new(HashMap::new())),
            message_history: Arc::new(RwLock::new(Vec::new())),
            server_start_time: Self::current_timestamp(),
        }
    }

    /// Set communication channels (called by network initializer)
    pub fn set_channels(
        &mut self,
        packet_receiver: Receiver<Packet>,
        controller_sender: Sender<DroneEvent>,
        controller_receiver: Receiver<DroneCommand>,
    ) {
        self.base.set_channels(packet_receiver, controller_sender, controller_receiver);
    }

    /// Add a drone neighbor
    pub async fn add_neighbor(&self, drone_id: NodeId, sender: Sender<Packet>) {
        self.base.add_neighbor(drone_id, sender).await;
    }

    /// Start the server
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting Communication Server {}", self.base.id);
        
        // Discover network topology
        self.base.discover_network().await?;
        
        let packet_receiver = self.base.packet_receiver.take()
            .ok_or("Packet receiver not set")?;
        let controller_receiver = self.base.controller_receiver.take()
            .ok_or("Controller receiver not set")?;

        loop {
            select_biased! {
                recv(controller_receiver) -> command => {
                    if let Ok(command) = command {
                        self.handle_controller_command(command).await;
                    }
                },
                recv(packet_receiver) -> packet => {
                    if let Ok(packet) = packet {
                        if let Some(complete_message) = self.base.handle_packet(packet.clone()).await {
                            // Extract client ID from the packet's routing header
                            let client_id = packet.routing_header.hops.first().copied().unwrap_or(0);
                            if let Err(e) = self.process_client_message(client_id, complete_message).await {
                                error!("Error processing client message: {}", e);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Handle controller commands
    async fn handle_controller_command(&self, command: DroneCommand) {
        match command {
            DroneCommand::AddSender(drone_id, sender) => {
                self.base.add_neighbor(drone_id, sender).await;
            },
            DroneCommand::RemoveSender(drone_id) => {
                self.base.remove_neighbor(drone_id).await;
            },
            _ => {
                debug!("Communication Server received controller command: {:?}", command);
            }
        }
    }

    /// Process incoming client message
    async fn process_client_message(&self, client_id: NodeId, message_data: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        // Try to deserialize as ClientMessage
        let client_message: ClientMessage = ServerBase::deserialize_message(&message_data)?;
        debug!("Communication Server received message from {}: {:?}", client_id, client_message);

        let response = match client_message {
            ClientMessage::ServerTypeRequest => {
                info!("Client {} requested server type", client_id);
                ServerResponse::ServerType(ServerType::CommunicationServer)
            },
            ClientMessage::RegistrationToChat(client_name) => {
                info!("Client {} registering with name: {}", client_id, client_name);
                self.register_client(client_id, client_name).await
            },
            ClientMessage::ClientListRequest => {
                info!("Client {} requested client list", client_id);
                self.get_client_list().await
            },
            ClientMessage::MessageFor(target_client_name, message) => {
                info!("Client {} sending message to {}: {}", client_id, target_client_name, message);
                return self.forward_message(client_id, target_client_name, message).await;
            },
            _ => {
                warn!("Communication Server received unsupported request from {}: {:?}", client_id, client_message);
                ServerResponse::ErrorUnsupportedRequest
            }
        };

        // Serialize and send response back to the requesting client
        let response_data = ServerBase::serialize_message(&response)?;
        self.base.send_message(client_id, &response_data).await?;
        
        Ok(())
    }

    /// Register a new client
    async fn register_client(&self, client_id: NodeId, client_name: String) -> ServerResponse {
        // Check if name is already taken
        {
            let names = self.client_names.read().await;
            if names.contains_key(&client_name) {
                warn!("Client name '{}' is already taken", client_name);
                return ServerResponse::ErrorWrongClientId;
            }
        }

        // Register the client
        let now = Self::current_timestamp();
        let registered_client = RegisteredClient {
            id: client_id,
            name: client_name.clone(),
            registration_time: now,
            last_activity: now,
        };

        {
            let mut clients = self.registered_clients.write().await;
            clients.insert(client_id, registered_client);
        }

        {
            let mut names = self.client_names.write().await;
            names.insert(client_name.clone(), client_id);
        }

        info!("Successfully registered client {} with name '{}'", client_id, client_name);
        
        // Return current client list as confirmation
        self.get_client_list().await
    }

    /// Get list of registered clients
    async fn get_client_list(&self) -> ServerResponse {
        let clients = self.registered_clients.read().await;
        let client_names: Vec<String> = clients.values()
            .map(|client| client.name.clone())
            .collect();
        
        debug!("Returning client list: {:?}", client_names);
        ServerResponse::ClientList(client_names)
    }

    /// Forward message from one client to another
    async fn forward_message(
        &self,
        from_client_id: NodeId,
        target_client_name: String,
        message: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Find the target client
        let (target_client_id, from_client_name) = {
            let names = self.client_names.read().await;
            let clients = self.registered_clients.read().await;
            
            let target_id = match names.get(&target_client_name) {
                Some(&id) => id,
                None => {
                    warn!("Target client '{}' not found", target_client_name);
                    let error_response = ServerResponse::ErrorWrongClientId;
                    let response_data = ServerBase::serialize_message(&error_response)?;
                    self.base.send_message(from_client_id, &response_data).await?;
                    return Ok(());
                }
            };

            let from_name = clients.get(&from_client_id)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| format!("Client_{}", from_client_id));

            (target_id, from_name)
        };

        // Update last activity for sender
        {
            let mut clients = self.registered_clients.write().await;
            if let Some(client) = clients.get_mut(&from_client_id) {
                client.last_activity = Self::current_timestamp();
            }
        }

        // Create chat message for history
        let chat_message = ChatMessage {
            from_client_id,
            from_client_name: from_client_name.clone(),
            to_client_id: target_client_id,
            to_client_name: target_client_name.clone(),
            message: message.clone(),
            timestamp: Self::current_timestamp(),
        };

        // Store in message history
        {
            let mut history = self.message_history.write().await;
            history.push(chat_message);
            
            // Keep only last 1000 messages to prevent memory bloat
            if history.len() > 1000 {
                history.remove(0);
            }
        }

        // Forward message to target client
        let forwarded_message = ServerResponse::MessageFrom(from_client_name, message);
        let message_data = ServerBase::serialize_message(&forwarded_message)?;
        
        match self.base.send_message(target_client_id, &message_data).await {
            Ok(_) => {
                info!("Successfully forwarded message from {} to {}", from_client_id, target_client_name);
            },
            Err(e) => {
                error!("Failed to forward message to {}: {}", target_client_name, e);
                // Send error back to sender
                let error_response = ServerResponse::ErrorWrongClientId;
                let response_data = ServerBase::serialize_message(&error_response)?;
                self.base.send_message(from_client_id, &response_data).await?;
            }
        }

        Ok(())
    }

    /// Get current timestamp (mock implementation)
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Get server statistics
    pub async fn get_stats(&self) -> CommunicationStats {
        let clients = self.registered_clients.read().await;
        let history = self.message_history.read().await;
        
        let total_clients = clients.len();
        let active_clients = clients.values()
            .filter(|c| Self::current_timestamp() - c.last_activity < 300) // Active within 5 minutes
            .count();
        let total_messages = history.len();
        
        // Calculate messages per client
        let mut message_counts = HashMap::new();
        for msg in history.iter() {
            *message_counts.entry(msg.from_client_name.clone()).or_insert(0) += 1;
        }

        CommunicationStats {
            server_uptime: Self::current_timestamp() - self.server_start_time,
            total_clients,
            active_clients,
            total_messages,
            message_counts,
        }
    }

    /// Get registered clients
    pub async fn get_registered_clients(&self) -> Vec<RegisteredClient> {
        let clients = self.registered_clients.read().await;
        clients.values().cloned().collect()
    }

    /// Get message history
    pub async fn get_message_history(&self, limit: Option<usize>) -> Vec<ChatMessage> {
        let history = self.message_history.read().await;
        let start_index = if let Some(limit) = limit {
            if history.len() > limit {
                history.len() - limit
            } else {
                0
            }
        } else {
            0
        };
        
        history[start_index..].to_vec()
    }

    /// Remove a client (for cleanup or administrative purposes)
    pub async fn remove_client(&self, client_id: NodeId) -> bool {
        let client_name = {
            let mut clients = self.registered_clients.write().await;
            if let Some(client) = clients.remove(&client_id) {
                Some(client.name)
            } else {
                None
            }
        };

        if let Some(name) = client_name {
            let mut names = self.client_names.write().await;
            names.remove(&name);
            info!("Removed client {} ({})", client_id, name);
            true
        } else {
            false
        }
    }

    /// Check if a client is registered
    pub async fn is_client_registered(&self, client_id: NodeId) -> bool {
        let clients = self.registered_clients.read().await;
        clients.contains_key(&client_id)
    }

    /// Get client by name
    pub async fn get_client_by_name(&self, name: &str) -> Option<RegisteredClient> {
        let names = self.client_names.read().await;
        let clients = self.registered_clients.read().await;
        
        if let Some(&client_id) = names.get(name) {
            clients.get(&client_id).cloned()
        } else {
            None
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommunicationStats {
    pub server_uptime: u64,
    pub total_clients: usize,
    pub active_clients: usize,
    pub total_messages: usize,
    pub message_counts: HashMap<String, usize>,
}