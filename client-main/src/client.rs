use wg_internal::network::{NodeId, SourceRoutingHeader};
use wg_internal::packet::{Packet, FloodRequest, PacketType, NodeType};
use crossbeam::channel::{Receiver, Sender};
use std::collections::HashMap;
use std::sync::Arc;
use common::network::{Network, Node};
use common::types::{ClientType, ClientError};
use tokio::sync::{Mutex, RwLock};
use crate::assembler::Assembler;
use crate::web_browser::WebBrowserClient;
use crate::chat_client::ChatClient;

/// Main Client structure that handles the core networking and delegates to specific client types
pub struct Client {
    id: NodeId,
    client_name: String,
    client_type: ClientType,
    neighbors: Arc<Mutex<HashMap<NodeId, Sender<Packet>>>>,
    network_view: Arc<RwLock<Network>>,
    session_counter: Arc<Mutex<u64>>,
    assembler: Arc<Assembler>,
    web_browser: Option<Arc<WebBrowserClient>>,
    chat_client: Option<Arc<ChatClient>>,
    // Simulation controller communication
    controller_sender: Option<Sender<ClientEvent>>,
    controller_receiver: Option<Receiver<ClientCommand>>,
}

/// Commands from simulation controller to client
#[derive(Debug)]
pub enum ClientCommand {
    AddSender(NodeId, Sender<Packet>),
    RemoveSender(NodeId),
    Shutdown,
}

/// Events from client to simulation controller
#[derive(Debug)]
pub enum ClientEvent {
    PacketSent(Packet),
    ClientRegistered(String),
    MessageSent(String, String), // (to, message)
}

impl Client {
    pub fn new(
        id: NodeId, 
        client_name: String,
        client_type: ClientType,
    ) -> Self {
        // Initialize network with self as root node
        let network_view = Arc::new(RwLock::new(Network::new(
            Node::new(id, NodeType::Client, vec![])
        )));
        
        let neighbors = Arc::new(Mutex::new(HashMap::new()));
        let assembler = Arc::new(Assembler::new(id, HashMap::new()));
        
        // Create specific client implementations based on type
        let (web_browser, chat_client) = match client_type {
            ClientType::WebBrowser => {
                let web_client = Arc::new(WebBrowserClient::new(
                    id, 
                    assembler.clone(), 
                    network_view.clone()
                ));
                (Some(web_client), None)
            },
            ClientType::ChatClient => {
                let chat_client = Arc::new(ChatClient::new(
                    id, 
                    client_name.clone(), 
                    assembler.clone(), 
                    network_view.clone()
                ));
                (None, Some(chat_client))
            },
        };

        Self {
            id,
            client_name,
            client_type,
            neighbors,
            network_view,
            session_counter: Arc::new(Mutex::new(0)),
            assembler,
            web_browser,
            chat_client,
            controller_sender: None,
            controller_receiver: None,
        }
    }

    /// Start the client with main message processing loop
    pub async fn start(self, packet_receiver: Receiver<Packet>) -> Result<(), ClientError> {
        println!("🚀 Starting {} client '{}' (ID: {})", 
                 match self.client_type { 
                     ClientType::WebBrowser => "Web Browser", 
                     ClientType::ChatClient => "Chat" 
                 }, 
                 self.client_name, 
                 self.id);

        // Start packet processing task
        let packet_processor = self.start_packet_processor(packet_receiver).await;

        // Perform initial network discovery
        println!("🔍 Discovering network topology...");
        self.discover_network().await?;

        // Wait a bit for network discovery to complete
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Start the appropriate client application
        match self.client_type {
            ClientType::WebBrowser => {
                if let Some(web_browser) = &self.web_browser {
                    web_browser.browse_interactively().await?;
                }
            },
            ClientType::ChatClient => {
                if let Some(chat_client) = &self.chat_client {
                    chat_client.chat_interactively().await?;
                }
            },
        }

        // Wait for packet processor to complete
        packet_processor.await.map_err(|e| 
            ClientError::NetworkError(format!("Packet processor error: {}", e))
        )?;

        Ok(())
    }

    /// Start the packet processing task
    async fn start_packet_processor(&self, receiver: Receiver<Packet>) -> tokio::task::JoinHandle<()> {
        let assembler = self.assembler.clone();
        let network_view = self.network_view.clone();
        let client_id = self.id;
        let chat_client = self.chat_client.clone();
        
        tokio::task::spawn(async move {
            loop {
                match receiver.try_recv() {
                    Ok(packet) => {
                        if let Err(e) = Self::handle_packet(
                            packet, 
                            &assembler, 
                            &network_view, 
                            client_id,
                            &chat_client
                        ).await {
                            eprintln!("Error handling packet: {}", e);
                        }
                    },
                    Err(crossbeam::channel::TryRecvError::Empty) => {
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    },
                    Err(crossbeam::channel::TryRecvError::Disconnected) => {
                        println!("Packet receiver channel disconnected");
                        break;
                    },
                }
            }
        })
    }

    /// Handle incoming packets
    async fn handle_packet(
        packet: Packet,
        assembler: &Arc<Assembler>,
        network_view: &Arc<RwLock<Network>>,
        client_id: NodeId,
        chat_client: &Option<Arc<ChatClient>>,
    ) -> Result<(), ClientError> {
        match &packet.pack_type {
            PacketType::FloodResponse(flood_response) => {
                Self::handle_flood_response(flood_response, network_view).await
            },
            PacketType::MsgFragment(_) | PacketType::Ack(_) | PacketType::Nack(_) => {
                // Check if this is a chat message
                if let PacketType::MsgFragment(fragment) = &packet.pack_type {
                    // Try to deserialize as chat message first
                    if let Some(chat) = chat_client {
                        if let Ok(complete_data) = Self::try_assemble_fragment(packet.clone(), assembler).await {
                            if let Ok(chat_msg) = Self::try_parse_chat_message(&complete_data) {
                                let _ = chat.handle_incoming_message(
                                    chat_msg.from_client.unwrap_or_default(),
                                    chat_msg.message_content.unwrap_or_default()
                                ).await;
                                return Ok(());
                            }
                        }
                    }
                }
                
                // Default packet handling through assembler
                assembler.handle_packet(packet).await
            },
            _ => Ok(()), // Other packet types not handled by client
        }
    }

    /// Try to assemble a fragment into complete message
    async fn try_assemble_fragment(
        packet: Packet, 
        assembler: &Arc<Assembler>
    ) -> Result<Vec<u8>, ClientError> {
        // This is a simplified version - in reality, the assembler would handle this
        if let PacketType::MsgFragment(fragment) = packet.pack_type {
            if fragment.total_n_fragments == 1 {
                // Single fragment message
                return Ok(fragment.data[..fragment.length as usize].to_vec());
            }
        }
        Err(ClientError::FragmentationError("Multi-fragment assembly not implemented in demo".to_string()))
    }

    /// Try to parse data as chat message
    fn try_parse_chat_message(data: &[u8]) -> Result<crate::chat_client::ChatResponse, ClientError> {
        use crate::chat_client::ChatResponse;
        bincode::deserialize(data)
            .map_err(|_| ClientError::SerializationError("Not a chat message".to_string()))
    }

    /// Handle flood response to update network topology
    async fn handle_flood_response(
        flood_response: &wg_internal::packet::FloodResponse,
        network_view: &Arc<RwLock<Network>>,
    ) -> Result<(), ClientError> {
        let mut network = network_view.write().await;
        
        // Process path trace to update network topology
        for (i, &(node_id, node_type)) in flood_response.path_trace.iter().enumerate() {
            let mut neighbors = Vec::new();
            
            // Add previous node as neighbor
            if i > 0 {
                neighbors.push(flood_response.path_trace[i - 1].0);
            }
            
            // Add next node as neighbor
            if i + 1 < flood_response.path_trace.len() {
                neighbors.push(flood_response.path_trace[i + 1].0);
            }
            
            // Try to update existing node or add new one
            match network.update_node(node_id, neighbors.clone()) {
                Ok(_) => {},
                Err(_) => {
                    let _ = network.add_node(Node::new(node_id, node_type, neighbors));
                }
            }
        }
        
        Ok(())
    }

    /// Perform network discovery using flood protocol
    pub async fn discover_network(&self) -> Result<(), ClientError> {
        let session_id = {
            let mut counter = self.session_counter.lock().await;
            *counter += 1;
            *counter
        };

        let flood_request = FloodRequest::new(session_id, self.id);
        let packet = Packet::new_flood_request(
            SourceRoutingHeader::empty_route(),
            session_id,
            flood_request,
        );

        // Send flood request to all neighbors
        let neighbors = self.neighbors.lock().await;
        for (neighbor_id, sender) in neighbors.iter() {
            match sender.send(packet.clone()) {
                Ok(_) => println!("Sent network discovery to neighbor {}", neighbor_id),
                Err(e) => eprintln!("Failed to send discovery to neighbor {}: {:?}", neighbor_id, e),
            }
        }

        Ok(())
    }

    /// Add a neighbor connection (called by network initializer)
    pub async fn add_neighbor(&self, neighbor_id: NodeId, sender: Sender<Packet>) {
        let mut neighbors = self.neighbors.lock().await;
        neighbors.insert(neighbor_id, sender.clone());
        
        // Update assembler neighbors as well
        self.assembler.add_neighbor(neighbor_id, sender).await;
        
        println!("Added neighbor: {}", neighbor_id);
    }

    /// Remove a neighbor connection
    pub async fn remove_neighbor(&self, neighbor_id: NodeId) {
        let mut neighbors = self.neighbors.lock().await;
        neighbors.remove(&neighbor_id);
        
        self.assembler.remove_neighbor(neighbor_id).await;
        
        println!("Removed neighbor: {}", neighbor_id);
    }

    /// Get client ID
    pub fn get_id(&self) -> NodeId {
        self.id
    }

    /// Get client name
    pub fn get_name(&self) -> &str {
        &self.client_name
    }

    /// Get client type
    pub fn get_type(&self) -> &ClientType {
        &self.client_type
    }

    /// Set up simulation controller communication
    pub fn set_controller_communication(&mut self, 
        sender: Sender<ClientEvent>, 
        receiver: Receiver<ClientCommand>
    ) {
        self.controller_sender = Some(sender);
        self.controller_receiver = Some(receiver);
    }

    /// Handle simulation controller commands
    pub async fn handle_controller_commands(&self) -> Result<(), ClientError> {
        if let Some(receiver) = &self.controller_receiver {
            while let Ok(command) = receiver.try_recv() {
                match command {
                    ClientCommand::AddSender(node_id, sender) => {
                        self.add_neighbor(node_id, sender).await;
                    },
                    ClientCommand::RemoveSender(node_id) => {
                        self.remove_neighbor(node_id).await;
                    },
                    ClientCommand::Shutdown => {
                        println!("Client {} shutting down by controller command", self.id);
                        return Err(ClientError::NetworkError("Shutdown requested".to_string()));
                    },
                }
            }
        }
        Ok(())
    }

    /// Send event to simulation controller
    pub fn send_controller_event(&self, event: ClientEvent) {
        if let Some(sender) = &self.controller_sender {
            let _ = sender.send(event);
        }
    }
}