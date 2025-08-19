use std::any::Any;
use std::collections::{HashMap, HashSet};
use crossbeam::channel::{Receiver, Sender};
use wg_internal::network::NodeId;
use wg_internal::packet::{NodeType, Packet};
use common::{FragmentAssembler, RoutingHandler};
use common::packet_processor::Processor;
use common::types::{ChatCommand, ChatEvent, ChatRequest, ChatResponse, NodeCommand, ServerType};

pub struct ChatServer {
    routing_handler: RoutingHandler,
    controller_recv: Receiver<Box<dyn Any>>,
    controller_send: Sender<Box<dyn Any>>,
    packet_recv: Receiver<Packet>,
    id: NodeId,
    assembler: FragmentAssembler,
    registered_clients: HashSet<NodeId>,
}

impl ChatServer {
    pub fn new(id: NodeId, neighbors: HashMap<NodeId, Sender<Packet>>, packet_recv: Receiver<Packet>, controller_recv: Receiver<Box<dyn Any>>, controller_send: Sender<Box<dyn Any>>) -> Self {
        let router = RoutingHandler::new(id, NodeType::Server, neighbors, controller_send.clone());
        Self {
            routing_handler: router,
            controller_recv,
            controller_send,
            packet_recv,
            id,
            assembler: FragmentAssembler::default(),
            registered_clients: HashSet::new(),
        }
    }
    pub fn get_registered_clients(&self) -> Vec<NodeId> {
        self.registered_clients.iter().cloned().collect()
    }
}

impl Processor for ChatServer {
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

    fn handle_msg(&mut self, msg: Vec<u8>, from: NodeId, session_id: u64) {
        if let Ok(msg) = serde_json::from_slice::<ChatRequest>(&msg) {
            match msg {
                ChatRequest::ServerTypeQuery => {
                    if let Ok(res) = serde_json::to_vec(&ChatResponse::ServerType { server_type: ServerType::ChatServer }) {
                        let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                    }
                }
                ChatRequest::RegistrationToChat { client_id } => {
                    self.registered_clients.insert(client_id);
                    if let Ok(res) = serde_json::to_vec(&ChatResponse::RegistrationSuccess) {
                        let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                    }
                }
                ChatRequest::ClientListQuery => {
                    let client_list = self.registered_clients.iter().cloned().collect::<Vec<_>>();
                    if let Ok(res) = serde_json::to_vec(&ChatResponse::ClientList {list_of_client_ids: client_list}) {
                        let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                    }
                }
                ChatRequest::MessageFor { client_id, message } => {
                    if !self.registered_clients.contains(&client_id) {
                        if let Ok(res) = serde_json::to_vec(&ChatResponse::ErrorWrongClientId) {
                            let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                        }
                        return
                    }
                    if let Ok(res) = serde_json::to_vec(&ChatResponse::MessageFrom { client_id: from, message }) {
                        let _ = self.routing_handler.send_message(&res, client_id, Some(session_id));
                    }
                }
            }
        }
    }

    fn handle_command(&mut self, cmd: Box<dyn Any>) -> bool {
        if let Some(cmd) = cmd.downcast_ref::<NodeCommand>() {
            match cmd {
                NodeCommand::AddSender(node_id, sender) => self.routing_handler.add_neighbor(*node_id, sender.clone()),
                NodeCommand::RemoveSender(node_id) => self.routing_handler.remove_neighbor(*node_id),
                NodeCommand::Shutdown => return true
            }
        } else if let Some(cmd) = cmd.downcast_ref::<ChatCommand>() {
            match cmd {
                ChatCommand::GetRegisteredClients => {
                    let registered_clients = self.get_registered_clients();
                    if self.controller_send.send(Box::new(ChatEvent::RegisteredClients(registered_clients))).is_err() {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }
}

