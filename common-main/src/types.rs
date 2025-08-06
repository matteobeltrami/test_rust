use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex, RwLock};
use crossbeam::channel::Sender;
use wg_internal::packet::{Packet, PacketType};
use wg_internal::network::NodeId;

// Type aliases for better code organization
pub type PendingQueue = Arc<Mutex<HashMap<u64, oneshot::Sender<PacketType>>>>;
pub type SendingMap = Arc<RwLock<HashMap<NodeId, Sender<Packet>>>>;

// High-level message types for client-server communication
#[derive(Debug, Clone)]
pub enum ClientMessage {
    // Web Browser Messages
    ServerTypeRequest,
    FilesListRequest,
    FileRequest(String),
    MediaRequest(String),
    
    // Chat Client Messages  
    RegistrationToChat(String), // client name
    ClientListRequest,
    MessageFor(String, String), // (target_client, message)
}

#[derive(Debug, Clone)]
pub enum ServerResponse {
    // Web Server Responses
    ServerType(ServerType),
    FilesList(Vec<String>),
    File(usize, Vec<u8>), // (file_size, file_data)
    Media(Vec<u8>),
    ErrorRequestedNotFound,
    ErrorUnsupportedRequest,
    
    // Communication Server Responses
    ClientList(Vec<String>),
    MessageFrom(String, String), // (from_client, message)
    ErrorWrongClientId,
}

#[derive(Debug, Clone)]
pub enum ServerType {
    TextServer,
    MediaServer,
    CommunicationServer,
}

// Fragment reassembly state management
#[derive(Debug)]
pub struct FragmentAssembler {
    pub fragments: HashMap<u64, Vec<u8>>, // session_id -> data buffer
    pub expected_fragments: HashMap<u64, u64>, // session_id -> total_fragments
    pub received_fragments: HashMap<u64, Vec<bool>>, // session_id -> received status
}

impl FragmentAssembler {
    pub fn new() -> Self {
        Self {
            fragments: HashMap::new(),
            expected_fragments: HashMap::new(),
            received_fragments: HashMap::new(),
        }
    }

    pub fn add_fragment(&mut self, session_id: u64, fragment_index: u64, 
                       total_fragments: u64, length: u8, data: &[u8; 128]) -> Option<Vec<u8>> {
        // Initialize session if not exists
        if !self.fragments.contains_key(&session_id) {
            self.fragments.insert(session_id, vec![0u8; (total_fragments * 128) as usize]);
            self.expected_fragments.insert(session_id, total_fragments);
            self.received_fragments.insert(session_id, vec![false; total_fragments as usize]);
        }

        // Copy fragment data
        if let Some(buffer) = self.fragments.get_mut(&session_id) {
            let start_offset = (fragment_index * 128) as usize;
            let end_offset = start_offset + length as usize;
            buffer[start_offset..end_offset].copy_from_slice(&data[..length as usize]);
        }

        // Mark fragment as received
        if let Some(received) = self.received_fragments.get_mut(&session_id) {
            received[fragment_index as usize] = true;
        }

        // Check if all fragments received
        if let Some(received) = self.received_fragments.get(&session_id) {
            if received.iter().all(|&r| r) {
                // All fragments received, return complete message
                let complete_data = self.fragments.remove(&session_id)?;
                self.expected_fragments.remove(&session_id);
                self.received_fragments.remove(&session_id);
                
                // Calculate actual data length (remove trailing zeros from last fragment)
                let total_frags = self.expected_fragments.get(&session_id).unwrap_or(&1);
                let actual_length = if *total_frags > 1 {
                    (*total_frags - 1) * 128 + length as u64
                } else {
                    length as u64
                };
                
                return Some(complete_data[..actual_length as usize].to_vec());
            }
        }

        None
    }
}

// Message fragmentation utilities
pub struct MessageFragmenter;

impl MessageFragmenter {
    pub fn fragment_message(data: &[u8]) -> Vec<(u64, u64, u8, [u8; 128])> {
        let mut fragments = Vec::new();
        let total_fragments = (data.len() + 127) / 128; // Ceiling division
        
        for (index, chunk) in data.chunks(128).enumerate() {
            let mut fragment_data = [0u8; 128];
            fragment_data[..chunk.len()].copy_from_slice(chunk);
            
            fragments.push((
                index as u64,           // fragment_index
                total_fragments as u64, // total_n_fragments
                chunk.len() as u8,      // length
                fragment_data,          // data
            ));
        }
        
        fragments
    }
}

// Client application types
#[derive(Debug)]
pub enum ClientType {
    WebBrowser,
    ChatClient,
}

// Error types for client operations
#[derive(Debug)]
pub enum ClientError {
    NetworkError(String),
    FragmentationError(String),
    SerializationError(String),
    ProtocolError(String),
    TimeoutError,
    UnknownServer,
    InvalidResponse,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ClientError::FragmentationError(msg) => write!(f, "Fragmentation error: {}", msg),
            ClientError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            ClientError::ProtocolError(msg) => write!(f, "Protocol error: {}", msg),
            ClientError::TimeoutError => write!(f, "Operation timed out"),
            ClientError::UnknownServer => write!(f, "Unknown server"),
            ClientError::InvalidResponse => write!(f, "Invalid response from server"),
        }
    }
}

impl std::error::Error for ClientError {}