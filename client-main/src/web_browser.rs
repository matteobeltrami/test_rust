use std::collections::HashMap;
use wg_internal::network::NodeId;
use common::types::{ServerType, ClientError};
use crate::assembler::Assembler;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Web Browser Client implementation
pub struct WebBrowserClient {
    id: NodeId,
    assembler: Arc<Assembler>,
    network_view: Arc<RwLock<common::network::Network>>,
    discovered_servers: Arc<Mutex<HashMap<NodeId, ServerType>>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct WebRequest {
    message_type: String,
    file_id: Option<String>,
    media_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct WebResponse {
    response_type: String,
    server_type: Option<String>,
    files_list: Option<Vec<String>>,
    file_data: Option<Vec<u8>>,
    file_size: Option<usize>,
    media_data: Option<Vec<u8>>,
    error_message: Option<String>,
}

impl WebBrowserClient {
    pub fn new(
        id: NodeId, 
        assembler: Arc<Assembler>,
        network_view: Arc<RwLock<common::network::Network>>,
    ) -> Self {
        Self {
            id,
            assembler,
            network_view,
            discovered_servers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Discover all text and media servers in the network
    pub async fn discover_servers(&self) -> Result<Vec<NodeId>, ClientError> {
        let network = self.network_view.read().await;
        let mut servers = Vec::new();

        // Find all server nodes in the network
        for node in &network.nodes {
            if matches!(node.get_node_type(), wg_internal::packet::NodeType::Server) {
                servers.push(node.get_id());
            }
        }

        // Query each server for its type
        for &server_id in &servers {
            if let Ok(server_type) = self.get_server_type(server_id).await {
                let mut discovered = self.discovered_servers.lock().await;
                discovered.insert(server_id, server_type);
            }
        }

        Ok(servers)
    }

    /// Get the type of a specific server
    pub async fn get_server_type(&self, server_id: NodeId) -> Result<ServerType, ClientError> {
        let request = WebRequest {
            message_type: "server_type?".to_string(),
            file_id: None,
            media_id: None,
        };

        let route = self.find_route_to_server(server_id).await?;
        let response_data = self.assembler.send_message(&request, server_id, route).await?;
        
        if let Ok(response) = self.assembler.deserialize_message::<WebResponse>(&response_data) {
            match response.server_type.as_deref() {
                Some("text") => Ok(ServerType::TextServer),
                Some("media") => Ok(ServerType::MediaServer),
                Some("communication") => Ok(ServerType::CommunicationServer),
                _ => Err(ClientError::InvalidResponse),
            }
        } else {
            Err(ClientError::InvalidResponse)
        }
    }

    /// Get list of files from a text server
    pub async fn get_files_list(&self, server_id: NodeId) -> Result<Vec<String>, ClientError> {
        // Verify it's a text server
        let discovered = self.discovered_servers.lock().await;
        if let Some(ServerType::TextServer) = discovered.get(&server_id) {
            drop(discovered);

            let request = WebRequest {
                message_type: "files_list?".to_string(),
                file_id: None,
                media_id: None,
            };

            let route = self.find_route_to_server(server_id).await?;
            let response_data = self.assembler.send_message(&request, server_id, route).await?;
            
            if let Ok(response) = self.assembler.deserialize_message::<WebResponse>(&response_data) {
                response.files_list.ok_or(ClientError::InvalidResponse)
            } else {
                Err(ClientError::InvalidResponse)
            }
        } else {
            Err(ClientError::ProtocolError("Server is not a text server".to_string()))
        }
    }

    /// Get a specific file from a text server
    pub async fn get_file(&self, server_id: NodeId, file_id: &str) -> Result<(usize, Vec<u8>), ClientError> {
        let request = WebRequest {
            message_type: "file?".to_string(),
            file_id: Some(file_id.to_string()),
            media_id: None,
        };

        let route = self.find_route_to_server(server_id).await?;
        let response_data = self.assembler.send_message(&request, server_id, route).await?;
        
        if let Ok(response) = self.assembler.deserialize_message::<WebResponse>(&response_data) {
            match (response.file_size, response.file_data) {
                (Some(size), Some(data)) => Ok((size, data)),
                _ => {
                    if response.error_message.is_some() {
                        Err(ClientError::ProtocolError("File not found".to_string()))
                    } else {
                        Err(ClientError::InvalidResponse)
                    }
                }
            }
        } else {
            Err(ClientError::InvalidResponse)
        }
    }

    /// Get media from a media server
    pub async fn get_media(&self, server_id: NodeId, media_id: &str) -> Result<Vec<u8>, ClientError> {
        // Verify it's a media server
        let discovered = self.discovered_servers.lock().await;
        if let Some(ServerType::MediaServer) = discovered.get(&server_id) {
            drop(discovered);

            let request = WebRequest {
                message_type: "media?".to_string(),
                file_id: None,
                media_id: Some(media_id.to_string()),
            };

            let route = self.find_route_to_server(server_id).await?;
            let response_data = self.assembler.send_message(&request, server_id, route).await?;
            
            if let Ok(response) = self.assembler.deserialize_message::<WebResponse>(&response_data) {
                response.media_data.ok_or_else(|| {
                    if response.error_message.is_some() {
                        ClientError::ProtocolError("Media not found".to_string())
                    } else {
                        ClientError::InvalidResponse
                    }
                })
            } else {
                Err(ClientError::InvalidResponse)
            }
        } else {
            Err(ClientError::ProtocolError("Server is not a media server".to_string()))
        }
    }

    /// Display file content (simple text display)
    pub fn display_file(&self, file_data: &[u8]) -> Result<String, ClientError> {
        String::from_utf8(file_data.to_vec())
            .map_err(|_| ClientError::ProtocolError("Invalid file format".to_string()))
    }

    /// Interactive browser session
    pub async fn browse_interactively(&self) -> Result<(), ClientError> {
        println!("🌐 Web Browser Client Started");
        println!("Discovering servers...");
        
        let servers = self.discover_servers().await?;
        println!("Found {} servers", servers.len());

        // Display available text servers
        let discovered = self.discovered_servers.lock().await;
        let text_servers: Vec<NodeId> = discovered
            .iter()
            .filter_map(|(&id, server_type)| {
                if matches!(server_type, ServerType::TextServer) {
                    Some(id)
                } else {
                    None
                }
            })
            .collect();
        drop(discovered);

        if text_servers.is_empty() {
            println!("No text servers found!");
            return Ok(());
        }

        println!("\n📂 Available Text Servers:");
        for (index, &server_id) in text_servers.iter().enumerate() {
            println!("{}. Server {}", index + 1, server_id);
        }

        // For demo purposes, automatically browse the first server
        if let Some(&first_server) = text_servers.first() {
            println!("\n🔍 Browsing Server {}...", first_server);
            
            match self.get_files_list(first_server).await {
                Ok(files) => {
                    println!("📄 Available files:");
                    for (index, file) in files.iter().enumerate() {
                        println!("{}. {}", index + 1, file);
                    }

                    // Demo: Get first file
                    if let Some(first_file) = files.first() {
                        println!("\n📖 Retrieving file '{}'...", first_file);
                        match self.get_file(first_server, first_file).await {
                            Ok((size, data)) => {
                                println!("📄 File size: {} bytes", size);
                                match self.display_file(&data) {
                                    Ok(content) => {
                                        println!("📄 Content:\n{}", content);
                                    },
                                    Err(_) => {
                                        println!("📄 Binary content ({} bytes)", data.len());
                                    }
                                }
                            },
                            Err(e) => println!("❌ Error retrieving file: {}", e),
                        }
                    }
                },
                Err(e) => println!("❌ Error getting file list: {}", e),
            }
        }

        Ok(())
    }

    async fn find_route_to_server(&self, server_id: NodeId) -> Result<Vec<NodeId>, ClientError> {
        let network = self.network_view.read().await;
        network.find_path(server_id)
            .map_err(|_| ClientError::NetworkError("No route to server".to_string()))
    }
}