
use std::collections::{HashMap, HashSet};
use crossbeam::channel::{Receiver, Sender};
use wg_internal::network::NodeId;
use wg_internal::packet::{NodeType, Packet};
use common::{FragmentAssembler, RoutingHandler};
use common::packet_processor::Processor;
use common::types::{ChatCommand, ChatEvent, ChatRequest, ChatResponse, Command, Event, NodeCommand, NodeEvent, ServerType};

pub struct ChatServer {
    routing_handler: RoutingHandler,
    controller_recv: Receiver<Box<dyn Command>>,
    controller_send: Sender<Box<dyn Event>>,
    packet_recv: Receiver<Packet>,
    id: NodeId,
    assembler: FragmentAssembler,
    registered_clients: HashSet<NodeId>,
}

impl ChatServer {
    #[must_use]
    pub fn new(id: NodeId, neighbors: HashMap<NodeId, Sender<Packet>>, packet_recv: Receiver<Packet>, controller_recv: Receiver<Box<dyn Command>>, controller_send: Sender<Box<dyn Event>>) -> Self {
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
    #[must_use]
    pub fn get_registered_clients(&self) -> Vec<NodeId> {
        self.registered_clients.iter().copied().collect()
    }
}

impl Processor for ChatServer {
    fn controller_recv(&self) -> &Receiver<Box<dyn Command>> {
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
        let _ = self.controller_send.send(Box::new(NodeEvent::MessageReceived {
            notification_from: self.id,
            from
        }));
        if let Ok(msg) = serde_json::from_slice::<ChatRequest>(&msg) {
            match msg {
                ChatRequest::ServerTypeQuery => {
                    let _ = self.controller_send.send(Box::new(NodeEvent::ServerTypeQueried {
                        notification_from: self.id,
                        from
                    }));
                    if let Ok(res) = serde_json::to_vec(&ChatResponse::ServerType { server_type: ServerType::ChatServer }) {
                        let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                        let _ = self.controller_send.send(Box::new(NodeEvent::MessageSent {
                            notification_from: self.id,
                            to: from
                        }));
                    }
                }
                ChatRequest::RegistrationToChat { client_id } => {
                    self.registered_clients.insert(client_id);
                    if let Ok(res) = serde_json::to_vec(&ChatResponse::RegistrationSuccess) {
                        let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                        let _ = self.controller_send.send(Box::new(NodeEvent::MessageSent {
                            notification_from: self.id,
                            to: from
                        }));
                        let _ = self.controller_send.send(Box::new(ChatEvent::ClientRegistered {
                            client: client_id,
                            server: self.id
                        }));
                    }
                }
                ChatRequest::ClientListQuery => {
                    let _ = self.controller_send.send(Box::new(ChatEvent::ClientListQueried {
                        notification_from: self.id,
                        from
                    }));
                    let client_list = self.registered_clients.iter().copied().collect::<Vec<_>>();
                    if let Ok(res) = serde_json::to_vec(&ChatResponse::ClientList {list_of_client_ids: client_list}) {
                        let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                        let _ = self.controller_send.send(Box::new(NodeEvent::MessageSent {
                            notification_from: self.id,
                            to: from
                        }));
                    }
                }
                ChatRequest::MessageFor { client_id, message } => {
                    if !self.registered_clients.contains(&client_id) {
                        if let Ok(res) = serde_json::to_vec(&ChatResponse::ErrorWrongClientId {
                            wrong_id: client_id
                        }) {
                            let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                            let _ = self.controller_send.send(Box::new(NodeEvent::MessageSent {
                                notification_from: self.id,
                                to: from
                            }));
                        }
                        return
                    }
                    if let Ok(res) = serde_json::to_vec(&ChatResponse::MessageFrom { client_id: from, message }) {
                        let _ = self.routing_handler.send_message(&res, client_id, Some(session_id));
                        let _ = self.controller_send.send(Box::new(NodeEvent::MessageSent {
                            notification_from: self.id,
                            to: client_id
                        }));
                    }
                }
            }
        }
    }

    fn handle_command(&mut self, cmd: Box<dyn Command>) -> bool {
        let cmd = cmd.into_any();
        if let Some(cmd) = cmd.downcast_ref::<NodeCommand>() {
            match cmd {
                NodeCommand::AddSender(node_id, sender) => self.routing_handler.add_neighbor(*node_id, sender.clone()),
                NodeCommand::RemoveSender(node_id) => self.routing_handler.remove_neighbor(*node_id),
                NodeCommand::Shutdown => return true
            }
        } else if let Some(ChatCommand::GetRegisteredClients) = cmd.downcast_ref::<ChatCommand>() {
            let registered_clients = self.get_registered_clients();
            if self.controller_send.send(Box::new(ChatEvent::RegisteredClients {
                notification_from: self.id,
                list: registered_clients
            })).is_err() {
                return true;
            }
        
        }
        false
    }
}

mod communication_server_tests {
    #[allow(clippy::wildcard_imports)]
    use super::*;
    use crossbeam::channel::unbounded;

    fn create_test_chat_server() -> (ChatServer, Receiver<Packet>, Sender<Box<dyn Command>>) {
        let (controller_send, controller_recv) = unbounded();
        let (event_send, _event_recv) = unbounded::<Box<dyn Event>>();
        let (packet_send, packet_recv) = unbounded();
        let mut neighbors = HashMap::new();
        neighbors.insert(2, packet_send);

        let server = ChatServer::new(1, neighbors, packet_recv.clone(), controller_recv, event_send);
        (server, packet_recv, controller_send)
    }

    #[test]
    /// Tests the client registration handling and message forwarding
    fn test_client_registration_and_message_forwarding() {
        let (mut server, _, _) = create_test_chat_server();

        let reg_request1 = ChatRequest::RegistrationToChat { client_id: 10 };
        let reg_request2 = ChatRequest::RegistrationToChat { client_id: 11 };

        server.handle_msg(serde_json::to_vec(&reg_request1).unwrap(), 10, 100);
        server.handle_msg(serde_json::to_vec(&reg_request2).unwrap(), 11, 101);

        assert_eq!(server.registered_clients.len(), 2);
        assert!(server.registered_clients.contains(&10));
        assert!(server.registered_clients.contains(&11));

        let list_request = ChatRequest::ClientListQuery;
        server.handle_msg(serde_json::to_vec(&list_request).unwrap(), 10, 102);

        let message_request = ChatRequest::MessageFor {
            client_id: 11,
            message: "Hello from 10".to_string()
        };
        server.handle_msg(serde_json::to_vec(&message_request).unwrap(), 10, 103);

        let invalid_message = ChatRequest::MessageFor {
            client_id: 99,
            message: "This should fail".to_string()
        };
        server.handle_msg(serde_json::to_vec(&invalid_message).unwrap(), 10, 104);
    }

    #[test]
    /// Tests malformed message handling, it shouldn't panick
    fn test_malformed_message_handling() {
        let (mut server, _, _) = create_test_chat_server();

        let invalid_json = b"{ invalid json }".to_vec();
        server.handle_msg(invalid_json, 2, 123);

        let wrong_structure = serde_json::to_vec(&serde_json::json!({
            "not_a_chat_request": true
        })).unwrap();
        server.handle_msg(wrong_structure, 2, 124);

        assert_eq!(server.registered_clients.len(), 0);
    }
}
