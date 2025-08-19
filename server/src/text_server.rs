use std::any::Any;
use std::collections::{HashMap};
use crossbeam::channel::{Receiver, Sender};
use uuid::Uuid;
use wg_internal::network::NodeId;
use wg_internal::packet::{NodeType, Packet};
use common::{FragmentAssembler, RoutingHandler};
use common::packet_processor::Processor;
use common::types::{NodeCommand, ServerType, TextFile, WebCommand, WebRequest, WebResponse};

pub struct TextServer {
    routing_handler: RoutingHandler,
    controller_recv: Receiver<Box<dyn Any>>,
    controller_send: Sender<Box<dyn Any>>,
    packet_recv: Receiver<Packet>,
    id: NodeId,
    assembler: FragmentAssembler,
    stored_files: HashMap<Uuid, TextFile>,
}

impl TextServer {
    pub fn new(id: NodeId, neighbors: HashMap<NodeId, Sender<Packet>>, packet_recv: Receiver<Packet>, controller_recv: Receiver<Box<dyn Any>>, controller_send: Sender<Box<dyn Any>>) -> Self {
        let router = RoutingHandler::new(id, NodeType::Server, neighbors, controller_send.clone());
        Self {
            routing_handler: router,
            controller_recv,
            controller_send,
            packet_recv,
            id,
            assembler: FragmentAssembler::default(),
            stored_files: HashMap::new(),
        }
    }
}

impl Processor for TextServer {
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
        if let Ok(msg) = serde_json::from_slice::<WebRequest>(&msg) {
            match msg {
                WebRequest::ServerTypeQuery => {
                    if let Ok(res) = serde_json::to_vec(&WebResponse::ServerType { server_type: ServerType::TextServer }) {
                        let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                    }
                }
                WebRequest::TextFilesListQuery => {todo!()}
                WebRequest::FileQuery { .. } => {todo!()}
                WebRequest::MediaQuery { .. } => {todo!()}
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
        }  else if let Some(cmd) = cmd.downcast_ref::<WebCommand>() {
            match cmd {
                WebCommand::GetCachedFiles => {todo!()}
                WebCommand::GetFile(_) => {todo!()}
                WebCommand::GetTextFiles => {todo!()}
                WebCommand::GetTextFile(_) => {todo!()}
                WebCommand::GetMediaFiles => {todo!()}
                WebCommand::GetMediaFile(_) => {todo!()}
            }
        }
        false
    }
}