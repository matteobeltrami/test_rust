use wg_internal::{network::NodeId, packet::Packet};
use crossbeam_channel::Sender;

pub type Bytes = Vec<u8>;


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