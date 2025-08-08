use std::sync::Arc;
use crossbeam::channel::{Receiver, Sender};
use wg_internal::{
    network::NodeId,
    packet::Packet,
    controller::{DroneEvent, DroneCommand},
};
use common::types::{ClientMessage, ServerResponse, ServerType};
use log::{info, warn};
use crate::{Server, TextHandler, MediaHandler, errors::ServerError};

/// Operating mode for the content server
#[derive(Debug, Clone)]
pub enum ServerMode {
    /// Serves only text files (like original text-server)
    TextOnly,
    /// Serves only media files (like original media-server)
    MediaOnly,
    /// Serves both text and media files in one server instance
    Combined,
}

impl std::fmt::Display for ServerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerMode::TextOnly => write!(f, "Text Only"),
            ServerMode::MediaOnly => write!(f, "Media Only"),
            ServerMode::Combined => write!(f, "Combined (Text + Media)"),
        }
    }
}

/// Content Server - unified server for both text and media content
pub struct ContentServer {
    server: Server,
    text_handler: Option<TextHandler>,
    media_handler: Option<MediaHandler>,
    mode: ServerMode,
}

impl ContentServer {
    /// Create a new Content Server
    pub fn new(id: NodeId, mode: ServerMode) -> Self {
        let text_handler = match mode {
            ServerMode::TextOnly | ServerMode::Combined => Some(TextHandler::new()),
            ServerMode::MediaOnly => None,
        };

        let media_handler = match mode {
            ServerMode::MediaOnly | ServerMode::Combined => Some(MediaHandler::new()),
            ServerMode::TextOnly => None,
        };

        Self {
            server: Server::new(id),
            text_handler,
            media_handler,
            mode,
        }
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

    /// Get server mode
    pub fn get_mode(&self) -> &ServerMode {
        &self.mode
    }

    /// Start the content server
    pub async fn run(&mut self) -> Result<(), ServerError> {
        let text_handler = self.text_handler.as_ref().map(|h| Arc::new(h));
        let media_handler = self.media_handler.as_ref().map(|h| Arc::new(h));
        let mode = self.mode.clone();
        
        self.server.run_with_handler(move |client_id, client_message| {
            let text_handler = text_handler.clone();
            let media_handler = media_handler.clone();
            let mode = mode.clone();
            async move {
                Self::handle_client_message(client_id, client_message, text_handler, media_handler, mode).await
            }
        }).await
    }

    /// Handle incoming client messages based on server mode
    async fn handle_client_message(
        client_id: NodeId,
        client_message: ClientMessage,
        text_handler: Option<Arc<&TextHandler>>,
        media_handler: Option<Arc<&MediaHandler>>,
        mode: ServerMode,
    ) -> Result<ServerResponse, ServerError> {
        match &client_message {
            ClientMessage::ServerTypeRequest => {
                info!("Client {} requested server type", client_id);
                let server_type = match mode {
                    ServerMode::TextOnly => ServerType::TextServer,
                    ServerMode::MediaOnly => ServerType::MediaServer,
                    ServerMode::Combined => {
                        // In combined mode, we can report as text server since we handle both
                        // Clients can discover media capabilities through other means
                        ServerType::TextServer
                    }
                };
                Ok(ServerResponse::ServerType(server_type))
            },
            _ => {
                // Try handlers in order based on mode
                match mode {
                    ServerMode::TextOnly => {
                        if let Some(handler) = text_handler.as_ref() {
                            if let Some(response) = handler.handle_message(client_id, &client_message).await? {
                                return Ok(response);
                            }
                        }
                    },
                    ServerMode::MediaOnly => {
                        if let Some(handler) = media_handler.as_ref() {
                            if let Some(response) = handler.handle_message(client_id, &client_message).await? {
                                return Ok(response);
                            }
                        }
                    },
                    ServerMode::Combined => {
                        // Try text handler first
                        if let Some(handler) = text_handler.as_ref() {
                            if let Some(response) = handler.handle_message(client_id, &client_message).await? {
                                return Ok(response);
                            }
                        }
                        // Then try media handler
                        if let Some(handler) = media_handler.as_ref() {
                            if let Some(response) = handler.handle_message(client_id, &client_message).await? {
                                return Ok(response);
                            }
                        }
                    }
                }

                // No handler could process the request
                warn!("Content Server received unsupported request from {}: {:?}", client_id, client_message);
                Ok(ServerResponse::ErrorUnsupportedRequest)
            }
        }
    }

    /// Get text file IDs (if text handler is available)
    pub async fn get_file_ids(&self) -> Option<Vec<String>> {
        if let Some(handler) = &self.text_handler {
            Some(handler.get_file_ids().await)
        } else {
            None
        }
    }

    /// Get specific text file by ID (if text handler is available)
    pub async fn get_file(&self, file_id: &str) -> Option<crate::TextFile> {
        if let Some(handler) = &self.text_handler {
            handler.get_file(file_id).await
        } else {
            None
        }
    }

    /// Get media IDs (if media handler is available)
    pub async fn get_media_ids(&self) -> Option<Vec<String>> {
        if let Some(handler) = &self.media_handler {
            Some(handler.get_media_ids().await)
        } else {
            None
        }
    }

    /// Get specific media by ID (if media handler is available)
    pub async fn get_media(&self, media_id: &str) -> Option<crate::MediaFile> {
        if let Some(handler) = &self.media_handler {
            handler.get_media(media_id).await
        } else {
            None
        }
    }

    /// Get media statistics (if media handler is available)
    pub async fn get_media_stats(&self) -> Option<crate::MediaStats> {
        if let Some(handler) = &self.media_handler {
            Some(handler.get_stats().await)
        } else {
            None
        }
    }

    /// Add a text file (if text handler is available)
    pub async fn add_text_file(&self, file: crate::TextFile) -> Result<(), ServerError> {
        if let Some(handler) = &self.text_handler {
            handler.add_file(file).await;
            Ok(())
        } else {
            Err(ServerError::ConfigurationError("Text handler not available in current mode".to_string()))
        }
    }

    /// Add a media file (if media handler is available)
    pub async fn add_media_file(&self, media: crate::MediaFile) -> Result<(), ServerError> {
        if let Some(handler) = &self.media_handler {
            handler.add_media(media).await;
            Ok(())
        } else {
            Err(ServerError::ConfigurationError("Media handler not available in current mode".to_string()))
        }
    }
}