use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crossbeam::channel::{Receiver, Sender};
use wg_internal::{
    network::NodeId,
    packet::Packet,
    controller::{DroneEvent, DroneCommand},
};
use common::types::{ClientMessage, ServerResponse, ServerType};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use crate::{Server, errors::ServerError};

/// Text file with references to media
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextFile {
    pub id: String,
    pub title: String,
    pub content: String,
    pub media_references: Vec<String>, // IDs of referenced media files
}

/// Text Server implementation - serves text files with media references
pub struct TextServer {
    server: Server,
    files: Arc<RwLock<HashMap<String, TextFile>>>,
}

impl TextServer {
    /// Create a new Text Server with sample content
    pub fn new(id: NodeId) -> Self {
        let server = Self {
            server: Server::new(id),
            files: Arc::new(RwLock::new(HashMap::new())),
        };
        
        // Initialize with sample content
        let sample_files = Self::create_sample_files();
        let files_arc = server.files.clone();
        
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut files_lock = files_arc.write().await;
                for file in sample_files {
                    files_lock.insert(file.id.clone(), file);
                }
            });
        });
        
        server
    }

    /// Set communication channels (called by network initializer)
    pub fn set_channels(
        &mut self,
        packet_receiver: Receiver<Packet>,
        controller_sender: Sender<DroneEvent>,
        controller_receiver: Receiver<DroneCommand>,
    ) {
        self.server.set_channels(packet_receiver, controller_sender, controller_receiver);
    }

    /// Add a drone neighbor
    pub async fn add_neighbor(&self, drone_id: NodeId, sender: Sender<Packet>) {
        self.server.add_neighbor(drone_id, sender).await;
    }

    /// Get server ID
    pub fn get_id(&self) -> NodeId {
        self.server.get_id()
    }

    /// Start the server
    pub async fn run(&mut self) -> Result<(), ServerError> {
        let files = self.files.clone();
        
        self.server.run_with_handler(move |client_id, client_message| {
            let files = files.clone();
            async move {
                Self::handle_client_message(client_id, client_message, files).await
            }
        }).await
    }

    /// Handle incoming client messages
    async fn handle_client_message(
        client_id: NodeId,
        client_message: ClientMessage,
        files: Arc<RwLock<HashMap<String, TextFile>>>,
    ) -> Result<ServerResponse, ServerError> {
        match client_message {
            ClientMessage::ServerTypeRequest => {
                info!("Client {} requested server type", client_id);
                Ok(ServerResponse::ServerType(ServerType::TextServer))
            },
            ClientMessage::FilesListRequest => {
                info!("Client {} requested files list", client_id);
                let files_lock = files.read().await;
                let file_ids: Vec<String> = files_lock.keys().cloned().collect();
                Ok(ServerResponse::FilesList(file_ids))
            },
            ClientMessage::FileRequest(file_id) => {
                info!("Client {} requested file: {}", client_id, file_id);
                let files_lock = files.read().await;
                if let Some(file) = files_lock.get(&file_id) {
                    let file_data = bincode::serialize(file)
                        .map_err(|e| ServerError::SerializationError(e.to_string()))?;
                    Ok(ServerResponse::File(file_data.len(), file_data))
                } else {
                    warn!("File not found: {}", file_id);
                    Ok(ServerResponse::ErrorRequestedNotFound)
                }
            },
            _ => {
                warn!("Text Server received unsupported request from {}: {:?}", client_id, client_message);
                Ok(ServerResponse::ErrorUnsupportedRequest)
            }
        }
    }

    /// Create sample text files for demonstration
    fn create_sample_files() -> Vec<TextFile> {
        vec![
            TextFile {
                id: "welcome".to_string(),
                title: "Welcome to Our Drone Network".to_string(),
                content: r#"Welcome to our advanced drone communication network!

This system demonstrates cutting-edge networking technology using autonomous drones 
to create resilient communication infrastructure. Our network can adapt to changing 
conditions and provides reliable message delivery even in challenging environments.

Key Features:
- Source routing for efficient path selection
- Fragment reassembly for large messages
- Network discovery and topology adaptation
- Fault tolerance with multiple drone redundancy

The network consists of specialized nodes:
- Text servers (like this one) for content delivery
- Media servers for multimedia content
- Communication servers for real-time messaging
- Client applications for user interaction

This content references the following media files:
- network_diagram.png: Visual representation of our network topology
- demo_video.mp4: Demonstration of the system in action
- architecture_schema.svg: Technical architecture overview

Thank you for using our drone network system!"#.to_string(),
                media_references: vec![
                    "network_diagram.png".to_string(),
                    "demo_video.mp4".to_string(),
                    "architecture_schema.svg".to_string(),
                ],
            },
            TextFile {
                id: "technical_specs".to_string(),
                title: "Technical Specifications".to_string(),
                content: r#"Drone Network Technical Specifications

Network Protocol: Advanced Programming 2024/2025 Specification
Communication: Bidirectional crossbeam channels
Routing: Source routing with flood discovery
Fragmentation: 128-byte fragments with reassembly
Error Handling: ACK/NACK acknowledgment protocol

Server Types:
1. Content Servers
   - Text Server: Serves text files with media references
   - Media Server: Provides multimedia content
2. Communication Servers
   - Real-time message forwarding between clients
   - Client registration and discovery

Technical Implementation:
- Language: Rust 2021 edition
- Concurrency: Tokio async runtime with crossbeam channels
- Serialization: Bincode for efficient message encoding
- Threading: Each node runs on dedicated thread"#.to_string(),
                media_references: vec![
                    "performance_chart.png".to_string(),
                    "code_example.rs".to_string(),
                ],
            },
            TextFile {
                id: "getting_started".to_string(),
                title: "Getting Started Guide".to_string(),
                content: r#"Getting Started with the Drone Network

Step 1: Network Initialization
The network initializer reads a TOML configuration file that defines drone
positions, connections, and packet drop rates for simulation.

Step 2: Starting Components
The system starts drones, servers, and clients according to configuration.

Step 3: Communication
Messages are serialized, fragmented, and routed through the drone network
using source routing with reliable delivery protocols."#.to_string(),
                media_references: vec![
                    "setup_tutorial.mp4".to_string(),
                    "config_examples.zip".to_string(),
                ],
            },
            TextFile {
                id: "news".to_string(),
                title: "Latest Network News".to_string(),
                content: r#"Network Status Updates

Recent Developments:
- Enhanced fault tolerance with multi-path routing
- Improved fragment reassembly performance  
- New visualization features in simulation controller
- Extended media server capabilities

Current Network Statistics:
- Active Drones: 15 units across 10 team implementations
- Server Instances: Multiple text, media, and communication servers
- Message Throughput: Optimized for 128-byte fragments"#.to_string(),
                media_references: vec![
                    "network_stats.png".to_string(),
                    "team_showcase.mp4".to_string(),
                ],
            },
        ]
    }

    /// Get all file IDs
    pub async fn get_file_ids(&self) -> Vec<String> {
        let files = self.files.read().await;
        files.keys().cloned().collect()
    }

    /// Get a specific file by ID
    pub async fn get_file(&self, file_id: &str) -> Option<TextFile> {
        let files = self.files.read().await;
        files.get(file_id).cloned()
    }

    /// Add a new file
    pub async fn add_file(&self, file: TextFile) {
        let mut files = self.files.write().await;
        files.insert(file.id.clone(), file);
    }
}