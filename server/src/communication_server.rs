use std::any::Any;
use std::collections::HashMap;
use crossbeam::channel::{Receiver, Sender};
use wg_internal::network::NodeId;
use wg_internal::packet::{NodeType, Packet};
use common::{FragmentAssembler, RoutingHandler};
use common::packet_processor::Processor;
use common::types::NodeCommand;

pub struct ChatServer {
    routing_handler: RoutingHandler,
    controller_recv: Receiver<Box<dyn Any>>,
    packet_recv: Receiver<Packet>,
    id: NodeId,
    assembler: FragmentAssembler,
}

impl ChatServer {
    pub fn new(id: NodeId, neighbors: HashMap<NodeId, Sender<Packet>>, packet_recv: Receiver<Packet>, controller_recv: Receiver<Box<dyn Any>>, controller_send: Sender<Box<dyn Any>>) -> Self {
        let router = RoutingHandler::new(id, NodeType::Server, neighbors, controller_send);
        Self {
            routing_handler: router,
            controller_recv,
            packet_recv,
            id,
            assembler: FragmentAssembler::default(),
        }
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

    fn routing_header(&mut self) -> &mut RoutingHandler {
        &mut self.routing_handler
    }

    fn handle_msg(&mut self, _msg: Vec<u8>) {
        todo!()
    }

    fn handle_command(&mut self, cmd: Box<dyn Any>) {
        if let Ok(cmd) = cmd.downcast::<NodeCommand>() {
            match *cmd {
                NodeCommand::AddSender(_, _) => {}
                NodeCommand::RemoveSender(_) => {}
                NodeCommand::Shutdown => {}
            }
        }
    }
}

