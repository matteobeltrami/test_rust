use common::packet_processor::Processor;
use common::types::{
    ChatCommand, ChatEvent, ChatRequest, ChatResponse, Command, Event, Message, NodeCommand, ServerType
};
use common::{FragmentAssembler, RoutingHandler};
use crossbeam_channel::{Receiver, Sender};
use std::collections::{HashMap, HashSet};
use wg_internal::packet::NodeType;
use wg_internal::{network::NodeId, packet::Packet};

pub struct ChatClient {
    id: NodeId,
    routing_handler: RoutingHandler,
    controller_recv: Receiver<Box<dyn Command>>,
    controller_send: Sender<Box<dyn Event>>,
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
        controller_recv: Receiver<Box<dyn Command>>,
        controller_send: Sender<Box<dyn Event>>,
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

    fn handle_command(&mut self, cmd: Box<dyn Command>) -> bool {
        let cmd = cmd.into_any();
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

#[cfg(test)]
mod chat_client_tests {
    use super::*;
    use crossbeam::channel::unbounded;
    use common::types::{ChatResponse, ServerType, Message};

    fn create_test_chat_client() -> ChatClient {
        let (_controller_send, controller_recv) = unbounded();
        let (event_send, _event_recv) = unbounded();
        let (_, packet_recv) = unbounded();
        let neighbors = HashMap::new();

        ChatClient::new(1, neighbors, packet_recv, controller_recv, event_send)
    }

    #[test]
    /// Tests `ServerType` response handling (chat server being added to `HashSet`)
    fn test_server_type_response_handling() {
        let mut client = create_test_chat_client();

        let response = ChatResponse::ServerType {
            server_type: ServerType::ChatServer
        };
        let serialized = serde_json::to_vec(&response).unwrap();
        client.handle_msg(serialized, 5, 100);

        assert!(client.communication_servers.contains(&5));
    }

    #[test]
    /// Tests `ClientList` response handling (chat client being added to `HashSet`)
    fn test_client_list_response_handling() {
        let mut client = create_test_chat_client();

        let response = ChatResponse::ClientList {
            list_of_client_ids: vec![10, 11, 12]
        };
        let serialized = serde_json::to_vec(&response).unwrap();
        client.handle_msg(serialized, 5, 101);

        assert_eq!(client.registered_clients.len(), 3);
        assert!(client.registered_clients.contains(&10));
        assert!(client.registered_clients.contains(&11));
        assert!(client.registered_clients.contains(&12));
    }

    #[test]
    /// Tests `MessageFrom` reception and storage in `chat_history`
    fn test_message_reception_and_storage() {
        let mut client = create_test_chat_client();

        let response = ChatResponse::MessageFrom {
            client_id: 20,
            message: "Hello from client 20".to_string()
        };
        let serialized = serde_json::to_vec(&response).unwrap();
        client.handle_msg(serialized, 5, 102);

        assert!(client.chats_history.contains_key(&20));
        let messages = client.chats_history.get(&20).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].from, 20);
        assert_eq!(messages[0].to, 1);
        assert_eq!(messages[0].text, "Hello from client 20".to_string());
    }

    #[test]
    /// Tests `GetRegisteredClients`, `GetChatsHistory` and `SendMessage` commands handling
    fn test_command_handling() {
        let mut client = create_test_chat_client();

        let cmd = ChatCommand::GetRegisteredClients;
        let should_not_continue = client.handle_command(Box::new(cmd));
        assert!(should_not_continue, "Continued after GetRegisteredClients");

        client.registered_clients.insert(10);
        client.registered_clients.insert(11);
        let message = Message::new(1, 10, "Test message".to_string());
        client.insert_message(10, message);

        let cmd = ChatCommand::GetChatsHistory;
        let should_not_continue = client.handle_command(Box::new(cmd));
        assert!(should_not_continue, "Continued after GetChatsHistory");

        let message = Message::new(1, 10, "Outgoing message".to_string());
        let cmd = ChatCommand::SendMessage(message);
        let should_not_continue = client.handle_command(Box::new(cmd));
        assert!(should_not_continue, "Continued after SendMessage");
    }
}
