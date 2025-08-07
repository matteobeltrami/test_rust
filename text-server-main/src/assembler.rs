// Re-export the assembler from common
pub use common::types::{FragmentAssembler, MessageFragmenter};

use wg_internal::{
    network::{NodeId, SourceRoutingHeader},
    packet::{Packet, PacketType, Fragment},
};
use serde::{Serialize, de::DeserializeOwned};
use crate::errors::ServerError;

/// Server-specific assembler functionality
pub struct Assembler;

impl Assembler {
    /// Create a fragment packet for sending
    pub fn create_fragment_packet(
        session_id: u64,
        fragment_index: u64,
        total_fragments: u64,
        data: &[u8],
        routing_header: SourceRoutingHeader,
    ) -> Packet {
        let mut fragment_data = [0u8; 128];
        let length = std::cmp::min(data.len(), 128);
        fragment_data[..length].copy_from_slice(&data[..length]);

        let fragment = Fragment {
            fragment_index,
            total_n_fragments: total_fragments,
            length: length as u8,
            data: fragment_data,
        };

        Packet {
            pack_type: PacketType::MsgFragment(fragment),
            routing_header,
            session_id,
        }
    }

    /// Serialize and fragment a message for transmission
    pub fn prepare_message_for_transmission<T: Serialize>(
        message: &T,
        session_id: u64,
        route: Vec<NodeId>,
    ) -> Result<Vec<Packet>, ServerError> {
        // Serialize the message
        let serialized_data = bincode::serialize(message)
            .map_err(|e| ServerError::SerializationError(e.to_string()))?;

        // Fragment the data
        let fragments = MessageFragmenter::fragment_message(&serialized_data);
        
        // Create packets for each fragment
        let mut packets = Vec::new();
        for (fragment_index, total_fragments, length, data) in fragments {
            let routing_header = SourceRoutingHeader {
                hop_index: 1,
                hops: route.clone(),
            };

            let packet = Self::create_fragment_packet(
                session_id,
                fragment_index,
                total_fragments,
                &data[..length as usize],
                routing_header,
            );
            packets.push(packet);
        }

        Ok(packets)
    }

    /// Deserialize a complete message
    pub fn deserialize_complete_message<T: DeserializeOwned>(data: &[u8]) -> Result<T, ServerError> {
        bincode::deserialize(data)
            .map_err(|e| ServerError::SerializationError(e.to_string()))
    }

    /// Validate fragment integrity
    pub fn validate_fragment(fragment: &Fragment) -> Result<(), ServerError> {
        if fragment.fragment_index >= fragment.total_n_fragments {
            return Err(ServerError::ValidationError(
                format!("Fragment index {} exceeds total fragments {}", 
                        fragment.fragment_index, fragment.total_n_fragments)
            ));
        }

        if fragment.length > 128 {
            return Err(ServerError::ValidationError(
                format!("Fragment length {} exceeds maximum of 128 bytes", fragment.length)
            ));
        }

        Ok(())
    }
}