use crate::{
    network::{Network, NetworkError, Node},
    types::NodeEvent,
};
use crossbeam_channel::Sender;
use std::{any::Any, collections::{HashMap, HashSet}};
use wg_internal::{
    network::{NodeId, SourceRoutingHeader},
    packet::{Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet},
};

#[derive(Debug, Clone)]
struct Buffer {
    // represents packets which reached the destination
    packets_received: HashMap<(u64, NodeId), Vec<(bool, Packet)>>,
}


impl Buffer {
    fn new() -> Self {
        Self {
            packets_received: HashMap::new()
        }
    }

    fn insert(&mut self, packet: Packet, session_id: u64, from: NodeId) {
        let id = (session_id, from);
        if let Some(v) = self.packets_received.get_mut(&id) {
            v.push((false, packet));
        } else {
            let _ = self.packets_received.insert(id, vec![(false, packet)]);
        }
    }

    fn mark_as_received(&mut self, session_id: u64, fragment_index: u64, form: NodeId) {
        let id = (session_id, form);
        if let Some(f) = self.packets_received.get_mut(&id) {
            #[allow(clippy::cast_possible_truncation)]
            let index = fragment_index as usize;
            let ( _received, frag  )= &f[index];
            f[index] = (true, frag.clone());

            if f.iter().all(|(r, _)| *r) {
                // If all fragments are received, remove the session
                self.packets_received.remove(&id);
            }
        }
    }

    fn get_fragment_by_id(&mut self, session_id: u64, fragment_index: u64, from: NodeId) -> Option<Packet> {
        let id = (session_id, from);
        if let Some(session) = self.packets_received.get(&id) {
            #[allow(clippy::cast_possible_truncation)]
            session.get(fragment_index as usize).map(|(r, p)| if *r { None } else { Some(p.clone()) })?
        } else {
            None
        }
    }
}


#[derive(Debug, Clone)]
pub struct RoutingHandler {
    id: NodeId,
    network_view: Network,
    neighbors: HashMap<NodeId, Sender<Packet>>,
    flood_seen: HashSet<(u64, NodeId)>,
    session_counter: u64,
    flood_counter: u64,
    controller_send: Sender<Box<dyn Any>>,
    buffer: Buffer,
}

impl RoutingHandler {
    #[must_use]
    pub fn new(
        id: NodeId,
        node_type: NodeType,
        neighbors: HashMap<NodeId, Sender<Packet>>,
        controller_send: Sender<Box<dyn Any>>,
    ) -> Self {
        Self {
            id,
            network_view: Network::new(Node::new(id, node_type, vec![])),
            neighbors,
            session_counter: 0,
            flood_counter: 0,
            flood_seen: HashSet::new(),
            controller_send,
            buffer: Buffer::new(),
        }
    }

    /// Sends a packet to a specific neighbor and notifies the controller about the packet sent.
    /// # Errors
    /// Returns an error if sending the packet to the neighbor fails or if sending the event to the controller fails.
    fn send(&self, neighbor: &Sender<Packet>, packet: Packet) -> Result<(), NetworkError> {
        neighbor.send(packet.clone())?;
        self.controller_send
            .send(Box::new(NodeEvent::PacketSent(packet)))
            .map_err(|_e| NetworkError::ControllerDisconnected)?;
        Ok(())
    }

    /// Starts a flood by incrementing the session and flood counters,
    /// creating a flood request packet,
    /// sending it to all neighbors,
    /// and notifying the controller about the flood start.
    /// # Errors
    /// Returns an error if sending the packet to the controller fails or if sending to any neighbor fails.
    pub fn start_flood(&mut self) -> Result<(), NetworkError> {
        self.session_counter += 1;
        self.flood_counter += 1;
        let packet = Packet::new_flood_request(
            SourceRoutingHeader::empty_route(),
            self.session_counter,
            FloodRequest::new(self.flood_counter, self.id),
        );
        self.controller_send
            .send(Box::new(NodeEvent::FloodStarted(
                self.flood_counter,
                self.id,
            )))
            .map_err(|_| NetworkError::ControllerDisconnected)?;
        for (node_id, sender) in &self.neighbors.clone() {
            if sender.send(packet.clone()).is_err() {
                self.remove_neighbor(*node_id);
            }
        }
        Ok(())
    }

    /// Tries to remove the neighbor from the neighbors map and network view
    pub fn remove_neighbor(&mut self, node_id: NodeId) {
        #[allow(clippy::let_unit_value)]
        let _ = self.neighbors.remove(&node_id);
        self.network_view.remove_node(node_id);
    }

    /// Adds a new neighbor to the neighbors map and updates the network view
    pub fn add_neighbor(&mut self, node_id: NodeId, sender: Sender<Packet>) {
        let _ = self.neighbors.insert(node_id, sender);
        let _ = self.network_view.update_node(self.id, vec![node_id]);
    }

    pub fn handle_flood_response(&mut self, flood_response: &FloodResponse) {
        if flood_response.flood_id == self.flood_counter {
            self.update_network_view(&flood_response.path_trace);
        }
    }

    fn update_network_view(&mut self, path_trace: &[(NodeId, NodeType)]) {
        for (i, &(node_id, node_type)) in path_trace.iter().enumerate() {
            let mut neighbors = Vec::new();

            // Add previous node as neighbor
            if i > 0 {
                neighbors.push(path_trace[i - 1].0);
            }

            // Add next node as neighbor
            if i + 1 < path_trace.len() {
                neighbors.push(path_trace[i + 1].0);
            }

            // Try to update existing node or add new one
            if self
                .network_view
                .update_node(node_id, neighbors.clone())
                .is_err()
            {
                let new_node = Node::new(node_id, node_type, neighbors.clone());
                self.network_view.add_node(new_node);
            }
        }
    }

    /// Handles a flood request by checking if the flood has been seen before.
    /// If it has not been seen, it generates a flood response and sends it to the neighbors.
    /// If it has been seen, it forwards the flood request to the neighbors except for the previous hop.
    /// # Errors
    /// Returns an error if sending the packet fails or if the flood request is malformed.
    pub fn handle_flood_request(
        &mut self,
        mut flood_request: FloodRequest,
        session_id: u64,
    ) -> Result<(), NetworkError> {
        let prev_hop = flood_request
            .path_trace
            .last()
            .map_or(flood_request.initiator_id, |x| x.0);

        flood_request.path_trace.push((self.id, NodeType::Drone));

        let flood_session = (flood_request.flood_id, flood_request.initiator_id);

        self.update_network_view(&flood_request.path_trace);

        if !self.flood_seen.insert(flood_session) || self.neighbors.len() == 1 {
            // generate flood response
            let route = if let Some(path) = self.network_view.find_path(flood_request.initiator_id)
            {
                SourceRoutingHeader::new(path, 1)
            } else {
                let mut route: Vec<_> = flood_request
                    .path_trace
                    .clone()
                    .iter()
                    .map(|(id, _)| *id)
                    .rev()
                    .collect::<Vec<_>>();

                if route.last() != Some(&flood_request.initiator_id) {
                    route.push(flood_request.initiator_id);
                }

                SourceRoutingHeader::new(route, 1)
            };

            let flood_response = FloodResponse {
                flood_id: flood_request.flood_id,
                path_trace: flood_request.path_trace,
            };

            let packet = Packet::new_flood_response(route, session_id, flood_response);

            self.try_send(packet)?;

            return Ok(());
        }

        let srh = SourceRoutingHeader::new(vec![], 0);

        let new_flood_request = Packet::new_flood_request(srh, session_id, flood_request);

        for (neighbor_id, neighbor) in &self.neighbors {
            if *neighbor_id != prev_hop {
                neighbor.send(new_flood_request.clone())?;
            }
        }
        Ok(())
    }

    /// Handles a NACK packet by removing the neighbor if the NACK indicates an error in routing,
    /// starting a flood to find a new route, and retrying to send the packet if it exists in the buffer.
    /// # Errors
    /// Returns an error if sending the packet fails or if the packet is not found in the buffer.
    pub fn handle_nack(
        &mut self,
        nack: &Nack,
        session_id: u64,
        source_id: NodeId,
    ) -> Result<(), NetworkError> {
        match nack.nack_type {
            NackType::ErrorInRouting(id) => {
                self.remove_neighbor(id);
                self.start_flood()?;
            },

            NackType::Dropped => {},

            NackType::DestinationIsDrone => self
                .network_view
                .change_node_type(source_id, NodeType::Drone),

            NackType::UnexpectedRecipient(_) => todo!(),
        }

        self.retry_send(session_id, nack.fragment_index, source_id)?;

        Ok(())
    }

    /// Send a packet to the first hop in its route
    /// # Errors
    /// Returns an error if send fails
    fn send_packet_to_first_hop(&self, packet: Packet) -> Result<(), NetworkError> {
        if packet.routing_header.hops.len() > 1 {
            let first_hop = packet.routing_header.hops[1];
            if let Some(sender) = self.neighbors.get(&first_hop) {
                self.send(sender, packet)?;
            } else {
                return Err(NetworkError::NodeIsNotANeighbor(first_hop));
            }
        }
        Ok(())
    }

    /// Tries to send a packet to next hop until it succeeds or there are no more neighbors.
    /// If sending fails, it removes the neighbor, finds a new route and tries again.
    /// # Errors
    /// Returns an error if the packet has no destination, if there are no neighbors, or if sending fails.
    fn try_send(&mut self, mut packet: Packet) -> Result<(), NetworkError> {
        // A packet must have a destination
        let destination = packet
            .routing_header
            .destination()
            .ok_or(NetworkError::NoDestination)?;

        let mut packet_sent = false;
        while !packet_sent && !self.neighbors.is_empty() {
            match self.send_packet_to_first_hop(packet.clone()) {
                Ok(()) => {
                    packet_sent = true;
                }
                Err(NetworkError::SendError(_t)) => {
                    // If the first hop is not a neighbor, remove it and try again
                    if let Some(first_hop) = packet.routing_header.hops.get(1) {
                        self.remove_neighbor(*first_hop);
                        // remove neighbor and start flood
                        let route = self
                            .network_view
                            .find_path(destination)
                            .ok_or(NetworkError::PathNotFound(destination))?;
                        packet.routing_header = SourceRoutingHeader::new(route, 1).without_loops();

                    }
                }
                Err(e) => return Err(e),
            }
        }

        if self.neighbors.is_empty() {
            return Err(NetworkError::NoNeighborAssigned);
        }

        Ok(())
    }

    /// Sends a message by fragmenting it into 128-byte chunks and sending each chunk as a separate packet.
    /// # Errors
    /// Returns an error if the destination path cannot be found or if sending fails.
    pub fn send_message(
        &mut self,
        message: &[u8],
        destination: NodeId,
        session_id: Option<u64>,
    ) -> Result<(), NetworkError> {
        let chunks: Vec<&[u8]> = message.chunks(128).collect();
        let total_n_fragments = chunks.len();

        if session_id.is_none() {
            self.session_counter += 1;
        }
        let shr = SourceRoutingHeader::new(
            self.network_view
                .find_path(destination)
                .ok_or(NetworkError::PathNotFound(destination))?,
            1,
        )
        .without_loops();

        for (i, chunk) in chunks.into_iter().enumerate() {
            // Pad/truncate to exactly 128 bytes
            let mut arr = [0u8; 128];
            arr[..chunk.len()].copy_from_slice(chunk);

            let fragment = Fragment::new(i as u64, total_n_fragments as u64, arr);

            let packet = Packet::new_fragment(
                shr.clone(),
                if let Some(id) = session_id {
                    id
                } else {
                    self.session_counter
                },
                fragment,
            );

            self.try_send(packet.clone())?;
            let session_id = packet.session_id;
            self.buffer.insert(
                packet,
                session_id,
                self.id
            );
        }

        Ok(())
    }

    pub fn handle_ack(&mut self, ack: &Ack, session_id: u64, from: NodeId) {
        self.buffer.mark_as_received(session_id, ack.fragment_index, from);
    }

    /// Retries sending a specific packet identified by `session_id` and `fragment_index` from a specific node.
    /// If the packet is found in the buffer, it is sent again.
    /// # Errors
    /// Returns an error if sending fails.
    pub fn retry_send(
        &mut self,
        session_id: u64,
        fragment_index: u64,
        from: NodeId,
    ) -> Result<(), NetworkError> {
        if let Some(packet) = self
            .buffer
            .get_fragment_by_id(session_id, fragment_index, from)
        {
            self.try_send(packet)?;
        }
        Ok(())
    }

    /// Sends an acknowledgment packet for a specific session and fragment index.
    /// The acknowledgment is sent to the source routing header (shr) provided.
    /// # Errors
    /// Returns an error if sending fails.
    pub fn send_ack(
        &mut self,
        shr: SourceRoutingHeader,
        session_id: u64,
        fragment_index: u64,
    ) -> Result<(), NetworkError> {
        let packet = Packet::new_ack(shr, session_id, fragment_index);
        self.try_send(packet)?;
        Ok(())
    }

    #[must_use]
    pub fn get_servers(&self) -> Option<Vec<NodeId>> {
        self.network_view.get_servers()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crossbeam_channel::unbounded;
    use wg_internal::packet::PacketType;

    #[test]
    fn test_add_neighbor() {
        let (sender, _receiver) = unbounded();
        let mut handler = RoutingHandler::new(1, NodeType::Client, HashMap::new(), sender);

        let (neighbor_sender, _neighbor_receiver) = unbounded();
        handler.add_neighbor(2, neighbor_sender);

        assert!(handler.neighbors.contains_key(&2));
        assert!(handler.network_view.nodes[0].get_adjacents().contains(&2));
    }

    #[test]
    fn test_remove_neighbor() {
        let (sender, _receiver) = unbounded();
        let mut handler = RoutingHandler::new(1, NodeType::Client, HashMap::new(), sender);

        let (neighbor_sender, _neighbor_receiver) = unbounded();
        handler.add_neighbor(2, neighbor_sender);
        handler.remove_neighbor(2);

        assert!(!handler.neighbors.contains_key(&2));
        assert!(!handler.network_view.nodes[0].get_adjacents().contains(&2));
    }

    #[test]
    fn test_start_flood() {
        let (sender, receiver) = unbounded();
        let mut handler = RoutingHandler::new(1, NodeType::Client, HashMap::new(), sender);

        let (neighbor_sender, neighbor_receiver) = unbounded();
        handler.add_neighbor(2, neighbor_sender);

        handler.start_flood().unwrap();

        let packet = receiver.try_recv().unwrap();
        if let Ok(cmd) = packet.downcast::<NodeEvent>() {

            assert!(matches!(*cmd, NodeEvent::FloodStarted(_, _)));

        }

        let neighbor_packet = neighbor_receiver.try_recv().unwrap();
        assert!(matches!(
            neighbor_packet.pack_type,
            PacketType::FloodRequest(_)
        ));
    }

    #[test]
    fn test_handle_flood_response() {
        let (sender, _receiver) = unbounded();
        let mut handler = RoutingHandler::new(1, NodeType::Client, HashMap::new(), sender);
        handler.flood_counter = 1;

        let flood_response = FloodResponse {
            flood_id: 1,
            path_trace: vec![(2, NodeType::Drone), (3, NodeType::Client)],
        };

        handler.handle_flood_response(&flood_response);

        assert!(handler.network_view.nodes.iter().any(|n| n.id == 2));
        assert!(handler.network_view.nodes.iter().any(|n| n.id == 3));
    }

    #[test]
    fn test_send_message() {
        let (sender, _receiver) = unbounded();
        let mut handler = RoutingHandler::new(1, NodeType::Client, HashMap::new(), sender);

        let (neighbor_sender, neighbor_receiver) = unbounded();
        handler.add_neighbor(2, neighbor_sender);

        let message = b"Hello world".to_vec(); // 128 bytes total

        handler.send_message(&message, 2, None).unwrap();

        let packet = neighbor_receiver.try_recv().unwrap();
        assert!(matches!(packet.pack_type, PacketType::MsgFragment(_)));
    }

    #[test]
    fn test_handle_ack() {
        let (sender, _receiver) = unbounded();
        let mut handler = RoutingHandler::new(1, NodeType::Client, HashMap::new(), sender);

        let (neighbor_sender, _neighbor_receiver) = unbounded();
        handler.add_neighbor(2, neighbor_sender);

        let message = b"Hello, world!".to_vec();
        handler.send_message(&message, 2, None).unwrap();

        let ack = Ack {
            fragment_index: 0,
        };

        handler.handle_ack(&ack, 1, 2);

    }
}
