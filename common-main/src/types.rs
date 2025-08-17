use serde::{Deserialize, Serialize};
use wg_internal::{network::NodeId, packet::Packet};
use crossbeam_channel::Sender;

pub type Bytes = Vec<u8>;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "message_type")]
pub enum ChatRequest {
    #[serde(rename = "server_type?")]
    ServerTypeQuery,

    #[serde(rename = "registration_to_chat")]
    RegistrationToChat { client_id: NodeId },

    #[serde(rename = "client_list?")]
    ClientListQuery,

    #[serde(rename = "message_for?")]
    MessageFor { client_id: NodeId, message: String },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "response_type")]
pub enum ChatResponse {
    #[serde(rename = "server_type!")]
    ServerTypeResponse { server_type: String },

    #[serde(rename = "client_list!")]
    ClientListResponse { list_of_client_ids: Vec<NodeId> },

    #[serde(rename = "message_from!")]
    MessageFrom { client_id: NodeId, message: String },

    #[serde(rename = "error_wrong_client_id!")]
    ErrorWrongClientId,

    // Custom response for successful registration
    #[serde(rename = "registration_success")]
    RegistrationSuccess,
}


pub enum ChatClientCommand {
    NodeCommand(NodeCommand)
}


#[derive(Debug, Clone)]
pub enum NodeEvent {
    PacketSent(Packet),
    FloodStarted(u64, NodeId),
    NodeRemoved(NodeId)
}


#[derive(Debug, Clone)]
pub enum NodeCommand {
    AddSender(NodeId, Sender<Packet>),
    RemoveSender(NodeId),
    Shutdown,
}

impl NodeCommand {
    #[must_use]
    pub fn as_add_sender(self) -> Option<(NodeId, Sender<Packet>)> {
        match self {
            NodeCommand::AddSender(id, sender) => Some((id, sender)),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_add_sender(&self) -> bool {
        matches!(self, Self::AddSender(_, _))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClientType {
    ChatClient,
    WebBrowser,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServerType {
    ChatServer,
    WebServer,
}