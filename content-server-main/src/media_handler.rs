use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use wg_internal::network::NodeId;
use common::types::{ClientMessage, ServerResponse, ServerType};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use crate::errors::ServerError;

/// Media file with metadata and content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFile {
    pub id: String,
    pub name: String,
    pub media_type: MediaType,
    pub size: usize,
    pub content: Vec<u8>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaType {
    Image(ImageFormat),
    Video(VideoFormat),  
    Audio(AudioFormat),
    Document(DocumentFormat),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageFormat {
    Png,
    Jpg,
    Svg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]  
pub enum VideoFormat {
    Mp4,
    Webm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioFormat {
    Mp3,
    Wav,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentFormat {
    Pdf,
    Txt,
    Zip,
    RustCode,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaStats {
    pub total_files: usize,
    pub total_size: usize,
    pub type_counts: HashMap<String, usize>,
}

/// Media handling functionality for the content server
pub struct MediaHandler {
    media_files: Arc<RwLock<HashMap<String, MediaFile>>>,
}

impl MediaHandler {
    /// Create a new Media Handler with sample content
    pub fn new() -> Self {
        let handler = Self {
            media_files: Arc::new(RwLock::new(HashMap::new())),
        };
        
        // Initialize with sample content
        let sample_media = Self::create_sample_media();
        let media_files_arc = handler.media_files.clone();
        
        tokio::spawn(async move {
            let mut files_lock = media_files_arc.write().await;
            for media in sample_media {
                files_lock.insert(media.id.clone(), media);
            }
        });
        
        handler
    }

    /// Handle media-related client messages
    pub async fn handle_message(
        &self,
        client_id: NodeId,
        client_message: &ClientMessage,
    ) -> Result<Option<ServerResponse>, ServerError> {
        match client_message {
            ClientMessage::ServerTypeRequest => {
                info!("Client {} requested server type", client_id);
                Ok(Some(ServerResponse::ServerType(ServerType::MediaServer)))
            },
            ClientMessage::MediaRequest(media_id) => {
                info!("Client {} requested media: {}", client_id, media_id);
                let media_files_lock = self.media_files.read().await;
                if let Some(media_file) = media_files_lock.get(media_id) {
                    info!("Serving media file: {} ({} bytes)", media_file.name, media_file.size);
                    Ok(Some(ServerResponse::Media(media_file.content.clone())))
                } else {
                    warn!("Media file not found: {}", media_id);
                    Ok(Some(ServerResponse::ErrorRequestedNotFound))
                }
            },
            _ => Ok(None), // Not handled by media handler
        }
    }

    /// Create sample media files for demonstration
    fn create_sample_media() -> Vec<MediaFile> {
        vec![
            MediaFile {
                id: "network_diagram.png".to_string(),
                name: "Network Topology Diagram".to_string(),
                media_type: MediaType::Image(ImageFormat::Png),
                size: 1024,
                content: Self::create_mock_image_data("PNG Network Diagram", 1024),
                description: "Visual representation of the drone network topology showing connections between nodes".to_string(),
            },
            MediaFile {
                id: "demo_video.mp4".to_string(),
                name: "System Demonstration Video".to_string(),
                media_type: MediaType::Video(VideoFormat::Mp4),
                size: 5120,
                content: Self::create_mock_video_data("Demo Video", 5120),
                description: "Complete walkthrough of the drone network system in action".to_string(),
            },
            MediaFile {
                id: "architecture_schema.svg".to_string(),
                name: "System Architecture Schema".to_string(),
                media_type: MediaType::Image(ImageFormat::Svg),
                size: 2048,
                content: Self::create_mock_svg_data("Architecture Schema", 2048),
                description: "Detailed technical architecture diagram showing all system components".to_string(),
            },
            MediaFile {
                id: "performance_chart.png".to_string(),
                name: "Performance Metrics Chart".to_string(),
                media_type: MediaType::Image(ImageFormat::Png),
                size: 1536,
                content: Self::create_mock_image_data("Performance Chart", 1536),
                description: "Real-time performance metrics showing throughput and latency data".to_string(),
            },
            MediaFile {
                id: "code_example.rs".to_string(),
                name: "Implementation Code Sample".to_string(),
                media_type: MediaType::Document(DocumentFormat::RustCode),
                size: 3072,
                content: Self::create_rust_code_example(),
                description: "Rust code examples showing key implementation patterns".to_string(),
            },
            MediaFile {
                id: "setup_tutorial.mp4".to_string(),
                name: "Setup Tutorial Video".to_string(),
                media_type: MediaType::Video(VideoFormat::Mp4),
                size: 7680,
                content: Self::create_mock_video_data("Setup Tutorial", 7680),
                description: "Step-by-step video guide for setting up the drone network".to_string(),
            },
            MediaFile {
                id: "config_examples.zip".to_string(),
                name: "Configuration Examples Archive".to_string(),
                media_type: MediaType::Document(DocumentFormat::Zip),
                size: 2560,
                content: Self::create_mock_config_archive(),
                description: "Collection of sample TOML configuration files for various network topologies".to_string(),
            },
            MediaFile {
                id: "network_stats.png".to_string(),
                name: "Current Network Statistics".to_string(),
                media_type: MediaType::Image(ImageFormat::Png),
                size: 1792,
                content: Self::create_mock_image_data("Network Stats", 1792),
                description: "Live dashboard showing current network performance and node status".to_string(),
            },
            MediaFile {
                id: "team_showcase.mp4".to_string(),
                name: "Team Implementation Showcase".to_string(),
                media_type: MediaType::Video(VideoFormat::Mp4),
                size: 6144,
                content: Self::create_mock_video_data("Team Showcase", 6144),
                description: "Highlights and demonstrations from all participating team implementations".to_string(),
            },
        ]
    }

    /// Create mock image data for demonstration
    fn create_mock_image_data(title: &str, size: usize) -> Vec<u8> {
        let header = format!("PNG_IMAGE_DATA:{}", title);
        let mut data = header.as_bytes().to_vec();
        
        // Pad with mock image data
        while data.len() < size {
            data.extend_from_slice(b"PIXEL_DATA_CHUNK_");
        }
        data.truncate(size);
        data
    }

    /// Create mock video data for demonstration
    fn create_mock_video_data(title: &str, size: usize) -> Vec<u8> {
        let header = format!("MP4_VIDEO_DATA:{}", title);
        let mut data = header.as_bytes().to_vec();
        
        // Pad with mock video frames
        while data.len() < size {
            data.extend_from_slice(b"VIDEO_FRAME_DATA_");
        }
        data.truncate(size);
        data
    }

    /// Create mock SVG data
    fn create_mock_svg_data(title: &str, size: usize) -> Vec<u8> {
        let svg_content = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 800 600\">\
              <title>{}</title>\
              <rect width=\"800\" height=\"600\" fill=\"#f0f0f0\"/>\
              <circle cx=\"400\" cy=\"300\" r=\"100\" fill=\"#007bff\"/>\
              <text x=\"400\" y=\"310\" text-anchor=\"middle\" font-size=\"16\" font-family=\"Arial\">\
                Content Server Architecture\
              </text>\
            </svg>", 
            title
        );
        
        let mut data = svg_content.as_bytes().to_vec();
        while data.len() < size {
            data.extend_from_slice(b"  <rect width=\"10\" height=\"10\" fill=\"#ccc\"/>\n");
        }
        data.truncate(size);
        data
    }

    /// Create Rust code example content
    fn create_rust_code_example() -> Vec<u8> {
        let code = r#"// Content Server - Unified text and media handling

use crossbeam::channel::{Receiver, Sender, select_biased};
use wg_internal::packet::{Packet, PacketType, Fragment};
use wg_internal::network::{NodeId, SourceRoutingHeader};

pub struct ContentServer {
    id: NodeId,
    text_handler: TextHandler,
    media_handler: MediaHandler,
    mode: ServerMode,
}

#[derive(Debug, Clone)]
pub enum ServerMode {
    TextOnly,
    MediaOnly,
    Combined,
}

impl ContentServer {
    pub fn new(id: NodeId, mode: ServerMode) -> Self {
        Self {
            id,
            text_handler: TextHandler::new(),
            media_handler: MediaHandler::new(),
            mode,
        }
    }

    pub async fn handle_request(&self, request: ClientMessage) -> Result<ServerResponse, ServerError> {
        match self.mode {
            ServerMode::TextOnly => self.text_handler.handle_message(&request).await,
            ServerMode::MediaOnly => self.media_handler.handle_message(&request).await,
            ServerMode::Combined => {
                // Try text handler first, then media handler
                if let Some(response) = self.text_handler.handle_message(&request).await? {
                    Ok(response)
                } else if let Some(response) = self.media_handler.handle_message(&request).await? {
                    Ok(response)
                } else {
                    Ok(ServerResponse::ErrorUnsupportedRequest)
                }
            }
        }
    }
}
"#;
        code.as_bytes().to_vec()
    }

    /// Create mock configuration archive data
    fn create_mock_config_archive() -> Vec<u8> {
        let config_content = r#"# Sample Network Configuration - Content Server

[[drone]]
id = 1
connected_node_ids = [2, 3]
pdr = 0.1

[[client]]
id = 100
connected_drone_ids = [1]

[[server]]
id = 200  # Content Server (combined text+media)
connected_drone_ids = [2, 3]
"#;
        let mut data = b"ZIP_ARCHIVE_HEADER".to_vec();
        data.extend_from_slice(config_content.as_bytes());
        data.extend_from_slice(b"ZIP_ARCHIVE_FOOTER");
        data
    }

    /// Get all media IDs
    pub async fn get_media_ids(&self) -> Vec<String> {
        let media_files = self.media_files.read().await;
        media_files.keys().cloned().collect()
    }

    /// Get specific media by ID
    pub async fn get_media(&self, media_id: &str) -> Option<MediaFile> {
        let media_files = self.media_files.read().await;
        media_files.get(media_id).cloned()
    }

    /// Add new media file
    pub async fn add_media(&self, media: MediaFile) {
        let mut media_files = self.media_files.write().await;
        media_files.insert(media.id.clone(), media);
    }

    /// Get media statistics
    pub async fn get_stats(&self) -> MediaStats {
        let media_files = self.media_files.read().await;
        let total_files = media_files.len();
        let total_size: usize = media_files.values().map(|m| m.size).sum();
        
        let mut type_counts = HashMap::new();
        for media in media_files.values() {
            let type_name = match &media.media_type {
                MediaType::Image(_) => "Image",
                MediaType::Video(_) => "Video", 
                MediaType::Audio(_) => "Audio",
                MediaType::Document(_) => "Document",
            };
            *type_counts.entry(type_name.to_string()).or_insert(0) += 1;
        }

        MediaStats {
            total_files,
            total_size,
            type_counts,
        }
    }
}