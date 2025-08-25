use common::packet_processor::Processor;
use common::types::{
    ChatCommand, ChatEvent, ChatRequest, ChatResponse, Command, Event, Message, NodeCommand,
    NodeEvent, ServerType,
};
use common::{FragmentAssembler, RoutingHandler};
use crossbeam_channel::{Receiver, Sender};
use std::collections::{HashMap, HashSet, VecDeque};
use wg_internal::packet::NodeType;
use wg_internal::{network::NodeId, packet::Packet};

pub struct ChatClient {
    id: NodeId,
    routing_handler: RoutingHandler,
    controller_recv: Receiver<Box<dyn Command>>,
    controller_send: Sender<Box<dyn Event>>,
    packet_recv: Receiver<Packet>,
    assembler: FragmentAssembler,
    registered_clients: HashMap<NodeId, Vec<NodeId>>, // server, list of clients registered to that
    // server
    pending_requests: VecDeque<ChatRequest>,
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
            registered_clients: HashMap::new(),
            communication_servers: HashSet::new(),
            chats_history: HashMap::new(),
            pending_requests: VecDeque::new(),
        }
    }

    fn get_chats_history(&self) -> HashMap<NodeId, Vec<Message>> {
        self.chats_history.clone()
    }

    fn add_list_of_registerd_clients(&mut self, server: NodeId, l: &[NodeId]) {
        if let Some(list) = self.registered_clients.get_mut(&server) {
            for n in l {
                if !list.contains(n) {
                    list.push(*n);
                }
            }
        } else {
            self.registered_clients.insert(server, l.to_vec());
        }
    }

    fn get_registered_clients(&self) -> Vec<NodeId> {
        let mut set = HashSet::new();
        for slice in self.registered_clients.values() {
            for n in slice {
                set.insert(*n);
            }
        }
        set.iter().copied().collect()
    }

    fn insert_message(&mut self, key: NodeId, message: Message) {
        if let Some(chat) = self.chats_history.get_mut(&key) {
            chat.push(message);
        } else {
            self.chats_history.insert(key, vec![message]);
        }
    }

    fn find_destination_by_client_id(&self, to: NodeId) -> Option<NodeId> {
        for (s, l) in &self.registered_clients {
            if l.contains(&to) {
                return Some(*s);
            }
        }
        None
    }

    fn discover_servers(&mut self) {
        let req = ChatRequest::ServerTypeQuery;
        if let Ok(req) = serde_json::to_vec(&req) {
            if let Some(servers) = self.routing_handler.get_servers() {
                for server in &servers {
                    let _ = self.routing_handler.send_message(&req, *server, None);
                }
            }
        }
    }

    fn broadcast(&mut self, req: &ChatRequest) {
        if let Ok(ser_req) = serde_json::to_vec(&req) {
            if self.communication_servers.is_empty() {
                self.pending_requests.push_back(req.clone());
                return;
            }
            for server in &self.communication_servers {
                let _ = self.routing_handler.send_message(&ser_req, *server, None);
            }
        }
    }

    fn handle_send_message(&mut self, message: &Message) -> bool {
        let req = ChatRequest::MessageFor {
            client_id: message.to,
            message: message.text.clone(),
        };
        if let Some(dest) = self.find_destination_by_client_id(message.to) {
            if let Ok(req) = serde_json::to_vec(&req) {
                let _ = self.routing_handler.send_message(&req, dest, None);

                if self
                    .controller_send
                    .send(Box::new(ChatEvent::MessageSent {
                        notification_from: self.id,
                        to: message.to,
                    }))
                    .is_err()
                {
                    return true;
                }
                self.insert_message(message.to, message.clone());
            }
        } else {
            self.pending_requests.push_back(req);
            self.broadcast(&ChatRequest::ClientListQuery);
        }

        false
    }

    fn send_request(&mut self, req: &ChatRequest, dest: NodeId) {
        if let Ok(ser) = serde_json::to_vec(&req) {
            let _ = self.routing_handler.send_message(&ser, dest, None);
        }
    }

    fn handle_get_clients_list(&mut self) -> bool {
        if self.registered_clients.is_empty() {
            self.broadcast(&ChatRequest::ClientListQuery);
        } else if self
            .controller_send
            .send(Box::new(ChatEvent::RegisteredClients {
                notification_from: self.id,
                list: self.get_registered_clients(),
            }))
            .is_err()
        {
            return true;
        }
        false
    }

    fn try_send_pending_requests(&mut self) {
        let pending_requests = self
            .pending_requests
            .drain(..)
            .rev()
            .collect::<Vec<ChatRequest>>();
        for p in &pending_requests {
            match p {
                ChatRequest::ClientListQuery => self.broadcast(p),
                ChatRequest::MessageFor { client_id, message } => {
                    let _ = self.handle_send_message(&Message::new(
                        self.id,
                        *client_id,
                        message.clone(),
                    ));
                }
                _ => {}
            }
        }
    }

    fn handle_get_chats_history(&mut self) -> bool {
        let history = self.get_chats_history();
        if self
            .controller_send
            .send(Box::new(ChatEvent::ChatHistory {
                notification_from: self.id,
                history,
            }))
            .is_err()
        {
            return true;
        }
        false
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
                ChatCommand::GetChatsHistory => return self.handle_get_chats_history(),
                ChatCommand::GetRegisteredClients => return self.handle_get_clients_list(),
                ChatCommand::SendMessage(message) => {
                    return self.handle_send_message(message);
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
        let _ = self
            .controller_send
            .send(Box::new(NodeEvent::MessageReceived {
                notification_from: self.id,
                from,
            }));
        if let Ok(msg) = serde_json::from_slice::<ChatResponse>(&msg) {
            match msg {
                ChatResponse::ServerType { server_type } => {
                    if matches!(server_type, ServerType::ChatServer) {
                        self.communication_servers.insert(from);
                        self.try_send_pending_requests();
                    }
                }
                ChatResponse::ClientList { list_of_client_ids } => {
                    self.add_list_of_registerd_clients(from, &list_of_client_ids);
                    let _ = self
                        .controller_send
                        .send(Box::new(ChatEvent::RegisteredClients {
                            notification_from: self.id,
                            list: self.get_registered_clients(),
                        }));
                    self.try_send_pending_requests();
                }
                ChatResponse::MessageFrom { client_id, message } => {
                    let received = Message::new(client_id, self.id, message);
                    let _ = self
                        .controller_send
                        .send(Box::new(ChatEvent::MessageReceived {
                            notification_from: self.id,
                            msg: received.clone(),
                        }));
                    self.insert_message(client_id, received);
                }
                ChatResponse::ErrorWrongClientId { wrong_id } => {
                    let _ = self
                        .controller_send
                        .send(Box::new(ChatEvent::ErrorClientNotFound {
                            notification_from: self.id,
                            location: from,
                            not_found: wrong_id,
                        }));
                }
                ChatResponse::RegistrationSuccess => {
                    let _ = self
                        .controller_send
                        .send(Box::new(ChatEvent::RegistrationSucceeded {
                            notification_from: self.id,
                            to: from,
                        }));
                }
            }
        }
    }
}

#[cfg(test)]
mod chat_client_tests {
    use super::*;
    use common::types::{ChatResponse, Message, ServerType};
    use crossbeam::channel::unbounded;

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
            server_type: ServerType::ChatServer,
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
            list_of_client_ids: vec![10, 11, 12],
        };
        let serialized = serde_json::to_vec(&response).unwrap();
        client.handle_msg(serialized, 5, 101);

        assert_eq!(client.registered_clients.len(), 1);
        assert!(client.registered_clients.contains_key(&5));
        assert!(client.registered_clients.get(&5).unwrap().contains(&10));
        assert!(client.registered_clients.get(&5).unwrap().contains(&11));
        assert!(client.registered_clients.get(&5).unwrap().contains(&12));
    }

    #[test]
    /// Tests `MessageFrom` reception and storage in `chat_history`
    fn test_message_reception_and_storage() {
        let mut client = create_test_chat_client();

        let response = ChatResponse::MessageFrom {
            client_id: 20,
            message: "Hello from client 20".to_string(),
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
        let should_continue = !client.handle_command(Box::new(cmd)); // request put in pending
        assert!(should_continue, "Continued after GetRegisteredClients");

        client.registered_clients.insert(2, vec![10, 11]);
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
