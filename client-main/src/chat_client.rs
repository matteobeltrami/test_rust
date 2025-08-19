use common::packet_processor::Processor;
use common::types::{
    ChatCommand, ChatEvent, ChatRequest, ChatResponse, Message, NodeCommand, ServerType,
};
use common::{FragmentAssembler, RoutingHandler};
use crossbeam_channel::{Receiver, Sender};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use wg_internal::packet::NodeType;
use wg_internal::{network::NodeId, packet::Packet};

pub struct ChatClient {
    id: NodeId,
    routing_handler: RoutingHandler,
    controller_recv: Receiver<Box<dyn Any>>,
    controller_send: Sender<Box<dyn Any>>,
    packet_recv: Receiver<Packet>,
    assembler: FragmentAssembler,
    registered_clients: HashSet<NodeId>,
    communication_servers: HashSet<NodeId>,
    chats_history: HashMap<NodeId, Vec<Message>>,
}

impl ChatClient {
    #[must_use]
    pub fn new(
        id: NodeId,
        neighbors: HashMap<NodeId, Sender<Packet>>,
        packet_recv: Receiver<Packet>,
        controller_recv: Receiver<Box<dyn Any>>,
        controller_send: Sender<Box<dyn Any>>,
    ) -> Self {
        let routing_handler =
            RoutingHandler::new(id, NodeType::Client, neighbors, controller_send.clone());

        Self {
            id,
            routing_handler,
            controller_recv,
            controller_send,
            packet_recv,
            assembler: FragmentAssembler::default(),
            registered_clients: HashSet::new(),
            communication_servers: HashSet::new(),
            chats_history: HashMap::new(),
        }
    }

    fn get_chats_history(&self) -> HashMap<NodeId, Vec<Message>> {
        self.chats_history.clone()
    }

    fn get_registered_clients(&self) -> Vec<NodeId> {
        self.registered_clients.iter().copied().collect()
    }

    fn insert_message(&mut self, key: NodeId, message: Message) {
        if let Some(chat) = self.chats_history.get_mut(&key) {
            chat.push(message);
        } else {
            self.chats_history
                .insert(key, vec![message]);
        }
    }
}

impl Processor for ChatClient {
    fn controller_recv(&self) -> &Receiver<Box<dyn Any>> {
        &self.controller_recv
    }

    fn packet_recv(&self) -> &Receiver<Packet> {
        &self.packet_recv
    }

    fn assembler(&mut self) -> &mut FragmentAssembler {
        &mut self.assembler
    }

    fn routing_handler(&mut self) -> &mut RoutingHandler {
        &mut self.routing_handler
    }

    fn handle_command(&mut self, cmd: Box<dyn Any>) -> bool {
        if let Some(cmd) = cmd.downcast_ref::<ChatCommand>() {
            match cmd {
                ChatCommand::GetChatsHistory => {
                    let history = self.get_chats_history();
                    if self
                        .controller_send
                        .send(Box::new(ChatEvent::ChatHistory(history))).is_err() {
                            return true;
                    }
                }
                ChatCommand::GetRegisteredClients => {
                    let list = self.get_registered_clients();
                    if self
                        .controller_send
                        .send(Box::new(ChatEvent::RegisteredClients(list)))
                        .is_err() {
                            return true;
                    }
                }
                ChatCommand::SendMessage(message) => {
                    if let Ok(req) = serde_json::to_vec(&ChatRequest::MessageFor {
                        client_id: message.to,
                        message: message.text.clone(),
                    }) {
                        let _ = self.routing_handler.send_message(&req, message.to, None);
                        if self.controller_send.send(Box::new(ChatEvent::MessageSent)).is_err() {
                            return true;
                        }
                        self.insert_message(message.to, message.clone());
                    }
                }
            }
        } else if let Some(cmd) = cmd.downcast_ref::<NodeCommand>() {
            match cmd {
                NodeCommand::AddSender(node_id, sender) => {
                    self.routing_handler.add_neighbor(*node_id, sender.clone());
                }
                NodeCommand::RemoveSender(node_id) => {
                    self.routing_handler.remove_neighbor(*node_id);
                }
                NodeCommand::Shutdown => {
                    return true;
                }
            }
        }
        false
    }


    fn handle_msg(&mut self, msg: Vec<u8>, from: NodeId, _session_id: u64) {
        if let Ok(msg) = serde_json::from_slice::<ChatResponse>(&msg) {
            match msg {
                ChatResponse::ServerType { server_type} => {
                    if matches!(server_type, ServerType::ChatServer) {
                        self.communication_servers.insert(from);
                    }
                }
                ChatResponse::ClientList { list_of_client_ids } => {
                    for client in &list_of_client_ids {
                        self.registered_clients.insert(*client);
                    }
                }
                ChatResponse::MessageFrom { client_id, message } => {
                    let received= Message::new(client_id, self.id, message);
                    let _ = self.controller_send.send(Box::new(ChatEvent::MessageReceived(received.clone())));
                    self.insert_message(client_id, received);
                }
                ChatResponse::ErrorWrongClientId => todo!(),
                ChatResponse::RegistrationSuccess => {}
            }
        }
    }
}
