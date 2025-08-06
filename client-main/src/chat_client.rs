use wg_internal::network::NodeId;
use common::types::{ServerType, ClientError};
use crate::assembler::Assembler;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Chat Client implementation
pub struct ChatClient {
    id: NodeId,
    client_name: String,
    assembler: Arc<Assembler>,
    network_view: Arc<RwLock<common::network::Network>>,
    communication_servers: Arc<Mutex<Vec<NodeId>>>,
    registered_server: Arc<Mutex<Option<NodeId>>>,
    received_messages: Arc<Mutex<Vec<(String, String)>>>, // (from, message)
}

#[derive(Serialize, Deserialize, Debug)]
struct ChatRequest {
    message_type: String,
    client_name: Option<String>,
    target_client: Option<String>,
    message_content: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChatResponse {
    pub response_type: String,
    pub client_list: Option<Vec<String>>,
    pub from_client: Option<String>,
    pub message_content: Option<String>,
    pub error_message: Option<String>,
}

impl ChatClient {
    pub fn new(
        id: NodeId,
        client_name: String,
        assembler: Arc<Assembler>,
        network_view: Arc<RwLock<common::network::Network>>,
    ) -> Self {
        Self {
            id,
            client_name,
            assembler,
            network_view,
            communication_servers: Arc::new(Mutex::new(Vec::new())),
            registered_server: Arc::new(Mutex::new(None)),
            received_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Discover communication servers in the network
    pub async fn discover_communication_servers(&self) -> Result<Vec<NodeId>, ClientError> {
        let network = self.network_view.read().await;
        let mut servers = Vec::new();

        // Find all server nodes
        for node in &network.nodes {
            if matches!(node.get_node_type(), wg_internal::packet::NodeType::Server) {
                // Check if it's a communication server by querying its type
                if let Ok(route) = network.find_path(node.get_id()) {
                    let request = ChatRequest {
                        message_type: "server_type?".to_string(),
                        client_name: None,
                        target_client: None,
                        message_content: None,
                    };

                    if let Ok(response_data) = self.assembler.send_message(&request, node.get_id(), route).await {
                        if let Ok(response) = self.assembler.deserialize_message::<ChatResponse>(&response_data) {
                            if response.response_type == "server_type!" && 
                               response.message_content.as_deref() == Some("communication") {
                                servers.push(node.get_id());
                            }
                        }
                    }
                }
            }
        }

        let mut comm_servers = self.communication_servers.lock().await;
        *comm_servers = servers.clone();
        
        Ok(servers)
    }

    /// Register with a communication server
    pub async fn register_to_chat(&self, server_id: NodeId) -> Result<(), ClientError> {
        let request = ChatRequest {
            message_type: "registration_to_chat".to_string(),
            client_name: Some(self.client_name.clone()),
            target_client: None,
            message_content: None,
        };

        let route = self.find_route_to_server(server_id).await?;
        let response_data = self.assembler.send_message(&request, server_id, route).await?;
        
        if let Ok(response) = self.assembler.deserialize_message::<ChatResponse>(&response_data) {
            if response.response_type == "registration_success" {
                let mut registered = self.registered_server.lock().await;
                *registered = Some(server_id);
                println!("✅ Successfully registered to chat server {}", server_id);
                Ok(())
            } else {
                Err(ClientError::ProtocolError("Registration failed".to_string()))
            }
        } else {
            // Assume successful registration if no explicit response
            let mut registered = self.registered_server.lock().await;
            *registered = Some(server_id);
            Ok(())
        }
    }

    /// Get list of clients registered on the communication server
    pub async fn get_client_list(&self) -> Result<Vec<String>, ClientError> {
        let registered_server = {
            let registered = self.registered_server.lock().await;
            registered.clone()
        };

        if let Some(server_id) = registered_server {
            let request = ChatRequest {
                message_type: "client_list?".to_string(),
                client_name: None,
                target_client: None,
                message_content: None,
            };

            let route = self.find_route_to_server(server_id).await?;
            let response_data = self.assembler.send_message(&request, server_id, route).await?;
            
            if let Ok(response) = self.assembler.deserialize_message::<ChatResponse>(&response_data) {
                response.client_list.ok_or(ClientError::InvalidResponse)
            } else {
                Err(ClientError::InvalidResponse)
            }
        } else {
            Err(ClientError::ProtocolError("Not registered to any server".to_string()))
        }
    }

    /// Send a message to another client
    pub async fn send_message(&self, target_client: &str, message: &str) -> Result<(), ClientError> {
        let registered_server = {
            let registered = self.registered_server.lock().await;
            registered.clone()
        };

        if let Some(server_id) = registered_server {
            let request = ChatRequest {
                message_type: "message_for?".to_string(),
                client_name: Some(self.client_name.clone()),
                target_client: Some(target_client.to_string()),
                message_content: Some(message.to_string()),
            };

            let route = self.find_route_to_server(server_id).await?;
            let _response_data = self.assembler.send_message(&request, server_id, route).await?;
            
            println!("💬 Message sent to {}: {}", target_client, message);
            Ok(())
        } else {
            Err(ClientError::ProtocolError("Not registered to any server".to_string()))
        }
    }

    /// Handle incoming messages (to be called by the main client message handler)
    pub async fn handle_incoming_message(&self, from_client: String, message: String) -> Result<(), ClientError> {
        println!("📨 New message from {}: {}", from_client, message);
        
        let mut messages = self.received_messages.lock().await;
        messages.push((from_client, message));
        
        Ok(())
    }

    /// Get all received messages
    pub async fn get_received_messages(&self) -> Vec<(String, String)> {
        let messages = self.received_messages.lock().await;
        messages.clone()
    }

    /// Interactive chat session
    pub async fn chat_interactively(&self) -> Result<(), ClientError> {
        println!("💬 Chat Client '{}' Started", self.client_name);
        println!("Discovering communication servers...");
        
        let servers = self.discover_communication_servers().await?;
        if servers.is_empty() {
            println!("❌ No communication servers found!");
            return Ok(());
        }

        println!("Found {} communication servers", servers.len());

        // Register to the first available server
        if let Some(&first_server) = servers.first() {
            println!("📝 Registering to server {}...", first_server);
            self.register_to_chat(first_server).await?;

            // Get client list
            println!("👥 Getting list of connected clients...");
            match self.get_client_list().await {
                Ok(clients) => {
                    println!("📋 Connected clients:");
                    for client in &clients {
                        if client != &self.client_name {
                            println!("  - {}", client);
                        }
                    }

                    // Demo: Send message to first other client
                    if let Some(first_other_client) = clients.iter().find(|&c| c != &self.client_name) {
                        let demo_message = format!("Hello from {}! 👋", self.client_name);
                        println!("\n📤 Sending demo message to {}...", first_other_client);
                        self.send_message(first_other_client, &demo_message).await?;
                    } else {
                        println!("ℹ️  No other clients to chat with. Sending a message to self for demo.");
                        let demo_message = "Hello from myself! This is a demo message.";
                        self.send_message(&self.client_name, demo_message).await?;
                    }
                },
                Err(e) => println!("❌ Error getting client list: {}", e),
            }

            // Show received messages
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let received = self.get_received_messages().await;
            if !received.is_empty() {
                println!("\n📬 Received messages:");
                for (from, msg) in received {
                    println!("  {} -> {}", from, msg);
                }
            }
        }

        println!("✅ Chat session completed");
        Ok(())
    }

    async fn find_route_to_server(&self, server_id: NodeId) -> Result<Vec<NodeId>, ClientError> {
        let network = self.network_view.read().await;
        network.find_path(server_id)
            .map_err(|_| ClientError::NetworkError("No route to server".to_string()))
    }
}