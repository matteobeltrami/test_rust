use crossbeam::channel::{Receiver, Sender, select_biased};
use wg_internal::{
    network::NodeId,
    packet::Packet,
    controller::{DroneEvent, DroneCommand},
};
use common::server_base::ServerBase;
use common::types::{ClientMessage, ServerResponse};
use log::{info, error, debug};
use crate::errors::ServerError;

/// Base server functionality shared by all server types
pub struct Server {
    pub base: ServerBase,
    pub id: NodeId,
}

impl Server {
    /// Create a new server instance
    pub fn new(id: NodeId) -> Self {
        Self {
            base: ServerBase::new(id),
            id,
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

    /// Get server ID
    pub fn get_id(&self) -> NodeId {
        self.id
    }

    /// Discover network topology
    pub async fn discover_network(&self) -> Result<(), ServerError> {
        self.base.discover_network().await
            .map_err(|e| ServerError::NetworkError(e.to_string()))
    }

    /// Handle controller commands
    pub async fn handle_controller_command(&self, command: DroneCommand) {
        match command {
            DroneCommand::AddSender(drone_id, sender) => {
                self.base.add_neighbor(drone_id, sender).await;
            },
            DroneCommand::RemoveSender(drone_id) => {
                self.base.remove_neighbor(drone_id).await;
            },
            _ => {
                debug!("Server {} received controller command: {:?}", self.id, command);
            }
        }
    }

    /// Process incoming packet and return complete message if available
    pub async fn handle_packet(&self, packet: Packet) -> Option<(NodeId, Vec<u8>)> {
        // Extract client ID from the packet's routing header
        let client_id = packet.routing_header.hops.first().copied().unwrap_or(0);
        
        if let Some(complete_message) = self.base.handle_packet(packet).await {
            Some((client_id, complete_message))
        } else {
            None
        }
    }

    /// Send response back to client
    pub async fn send_response(&self, client_id: NodeId, response: &ServerResponse) -> Result<(), ServerError> {
        let response_data = ServerBase::serialize_message(response)
            .map_err(|e| ServerError::SerializationError(e.to_string()))?;
        
        self.base.send_message(client_id, &response_data).await
            .map_err(|e| ServerError::NetworkError(e.to_string()))
    }

    /// Deserialize client message
    pub fn deserialize_client_message(data: &[u8]) -> Result<ClientMessage, ServerError> {
        ServerBase::deserialize_message(data)
            .map_err(|e| ServerError::SerializationError(e.to_string()))
    }

    /// Run the server with custom message handler
    pub async fn run_with_handler<F, Fut>(&mut self, mut message_handler: F) -> Result<(), ServerError>
    where
        F: FnMut(NodeId, ClientMessage) -> Fut,
        Fut: std::future::Future<Output = Result<ServerResponse, ServerError>>,
    {
        info!("Starting Server {}", self.id);
        
        // Discover network topology
        self.discover_network().await?;
        
        let packet_receiver = self.base.packet_receiver.take()
            .ok_or(ServerError::ConfigurationError("Packet receiver not set".to_string()))?;
        let controller_receiver = self.base.controller_receiver.take()
            .ok_or(ServerError::ConfigurationError("Controller receiver not set".to_string()))?;

        loop {
            select_biased! {
                recv(controller_receiver) -> command => {
                    if let Ok(command) = command {
                        self.handle_controller_command(command).await;
                    }
                },
                recv(packet_receiver) -> packet => {
                    if let Ok(packet) = packet {
                        if let Some((client_id, message_data)) = self.handle_packet(packet).await {
                            match Self::deserialize_client_message(&message_data) {
                                Ok(client_message) => {
                                    debug!("Server {} received message from {}: {:?}", 
                                           self.id, client_id, client_message);
                                    
                                    match message_handler(client_id, client_message).await {
                                        Ok(response) => {
                                            if let Err(e) = self.send_response(client_id, &response).await {
                                                error!("Error sending response to {}: {}", client_id, e);
                                            }
                                        },
                                        Err(e) => {
                                            error!("Error processing message from {}: {}", client_id, e);
                                        }
                                    }
                                },
                                Err(e) => {
                                    error!("Error deserializing message from {}: {}", client_id, e);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}