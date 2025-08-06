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

// Web Browser Protocol Messages according to AP-protocol.md
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "message_type")]
pub enum WebRequest {
    #[serde(rename = "server_type?")]
    ServerTypeQuery,
    
    #[serde(rename = "files_list?")]
    FilesListQuery,
    
    #[serde(rename = "file?")]
    FileQuery { file_id: String },
    
    #[serde(rename = "media?")]
    MediaQuery { media_id: String },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "response_type")]
pub enum WebResponse {
    #[serde(rename = "server_type!")]
    ServerTypeResponse { server_type: String },
    
    #[serde(rename = "files_list!")]
    FilesListResponse { files: Vec<String> },
    
    #[serde(rename = "file!")]
    FileResponse { file_size: usize, file_data: Vec<u8> },
    
    #[serde(rename = "media!")]
    MediaResponse { media_data: Vec<u8> },
    
    #[serde(rename = "error_requested_not_found!")]
    ErrorNotFound,
    
    #[serde(rename = "error_unsupported_request!")]
    ErrorUnsupportedRequest,
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
        let request = WebRequest::ServerTypeQuery;

        let route = self.find_route_to_server(server_id).await?;
        let response_data = self.assembler.send_message(&request, server_id, route).await?;
        
        if let Ok(response) = self.assembler.deserialize_message::<WebResponse>(&response_data) {
            match response {
                WebResponse::ServerTypeResponse { server_type } => {
                    match server_type.as_str() {
                        "text" => Ok(ServerType::TextServer),
                        "media" => Ok(ServerType::MediaServer),
                        "communication" => Ok(ServerType::CommunicationServer),
                        _ => Err(ClientError::InvalidResponse),
                    }
                },
                WebResponse::ErrorNotFound => Err(ClientError::ProtocolError("Server not found".to_string())),
                WebResponse::ErrorUnsupportedRequest => Err(ClientError::ProtocolError("Request not supported".to_string())),
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

            let request = WebRequest::FilesListQuery;

            let route = self.find_route_to_server(server_id).await?;
            let response_data = self.assembler.send_message(&request, server_id, route).await?;
            
            if let Ok(response) = self.assembler.deserialize_message::<WebResponse>(&response_data) {
                match response {
                    WebResponse::FilesListResponse { files } => Ok(files),
                    WebResponse::ErrorNotFound => Err(ClientError::ProtocolError("Files not found".to_string())),
                    WebResponse::ErrorUnsupportedRequest => Err(ClientError::ProtocolError("Request not supported".to_string())),
                    _ => Err(ClientError::InvalidResponse),
                }
            } else {
                Err(ClientError::InvalidResponse)
            }
        } else {
            Err(ClientError::ProtocolError("Server is not a text server".to_string()))
        }
    }

    /// Get a specific file from a text server
    pub async fn get_file(&self, server_id: NodeId, file_id: &str) -> Result<(usize, Vec<u8>), ClientError> {
        let request = WebRequest::FileQuery {
            file_id: file_id.to_string(),
        };

        let route = self.find_route_to_server(server_id).await?;
        let response_data = self.assembler.send_message(&request, server_id, route).await?;
        
        if let Ok(response) = self.assembler.deserialize_message::<WebResponse>(&response_data) {
            match response {
                WebResponse::FileResponse { file_size, file_data } => Ok((file_size, file_data)),
                WebResponse::ErrorNotFound => Err(ClientError::ProtocolError("File not found".to_string())),
                WebResponse::ErrorUnsupportedRequest => Err(ClientError::ProtocolError("Request not supported".to_string())),
                _ => Err(ClientError::InvalidResponse),
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

            let request = WebRequest::MediaQuery {
                media_id: media_id.to_string(),
            };

            let route = self.find_route_to_server(server_id).await?;
            let response_data = self.assembler.send_message(&request, server_id, route).await?;
            
            if let Ok(response) = self.assembler.deserialize_message::<WebResponse>(&response_data) {
                match response {
                    WebResponse::MediaResponse { media_data } => Ok(media_data),
                    WebResponse::ErrorNotFound => Err(ClientError::ProtocolError("Media not found".to_string())),
                    WebResponse::ErrorUnsupportedRequest => Err(ClientError::ProtocolError("Request not supported".to_string())),
                    _ => Err(ClientError::InvalidResponse),
                }
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
        // Use BFS to find path from self to server
        use std::collections::{VecDeque, HashSet};
        
        let network = self.network_view.read().await;
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut parent: std::collections::HashMap<NodeId, NodeId> = std::collections::HashMap::new();
        
        queue.push_back(self.id);
        visited.insert(self.id);
        
        while let Some(current) = queue.pop_front() {
            if current == server_id {
                // Reconstruct path
                let mut path = vec![server_id];
                let mut node = server_id;
                
                while let Some(&prev) = parent.get(&node) {
                    path.push(prev);
                    node = prev;
                }
                
                path.reverse();
                println!("📍 Found route to server {}: {:?}", server_id, path);
                return Ok(path);
            }
            
            // Find current node in network
            if let Some(network_node) = network.nodes.iter().find(|n| n.get_id() == current) {
                for &neighbor in network_node.get_adjacents() {
                    if !visited.contains(&neighbor) {
                        visited.insert(neighbor);
                        parent.insert(neighbor, current);
                        queue.push_back(neighbor);
                    }
                }
            }
        }
        
        Err(ClientError::NetworkError(format!("No route found to server {}", server_id)))
    }
}