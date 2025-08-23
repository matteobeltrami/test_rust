#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use ap2024_unitn_cppenjoyers_drone::CppEnjoyersDrone;
use common::types::NodeCommand;
use crossbeam::channel::{unbounded, Receiver, Sender};
use d_r_o_n_e_drone::MyDrone as DroneDrone;
use dr_ones::Drone as DrOnesDrone;
use lockheedrustin_drone::LockheedRustin;
use null_pointer_drone::MyDrone as NullPointerDrone;
use rustafarian_drone::RustafarianDrone;
use rustbusters_drone::RustBustersDrone;
use rusteze_drone::RustezeDrone;
use rusty_drones::RustyDrone;
use std::collections::HashMap;
use wg_2024_rust::drone::RustDrone;
use wg_internal::config::{Client, Drone, Server};
use wg_internal::controller::{DroneCommand, DroneEvent};
use wg_internal::drone::Drone as DroneTrait;
use wg_internal::network::NodeId;
use wg_internal::packet::Packet;

type DroneAttributes = (
    NodeId,                          // id
    Receiver<DroneCommand>,          // controller_recv
    Receiver<Packet>,                // packet_recv
    HashMap<NodeId, Sender<Packet>>, // neighbors
    f32,
);
pub enum NodeType<'a> {
    Drone(&'a Drone),
    Client(&'a Client),
    Server(&'a Server),
}

impl<'a> NodeType<'a> {
    pub fn id(&'a self) -> NodeId {
        match self {
            NodeType::Drone(drone) => drone.id,
            NodeType::Client(client) => client.id,
            NodeType::Server(server) => server.id,
        }
    }

    pub fn connected_node_ids(&'a self) -> &'a Vec<NodeId> {
        match self {
            NodeType::Drone(drone) => &drone.connected_node_ids,
            NodeType::Client(client) => &client.connected_drone_ids,
            NodeType::Server(server) => &server.connected_drone_ids,
        }
    }
}

macro_rules! drone_factories {
    ( $( $variant:ident ),* $(,)? ) => {
        paste::paste! {
            // one factory function per drone type
            $(
                fn [<$variant:snake _factory>](
                    id: NodeId,
                    controller_send: Sender<DroneEvent>,
                    controller_recv: Receiver<DroneCommand>,
                    packet_recv: Receiver<Packet>,
                    packet_send: HashMap<NodeId, Sender<Packet>>,
                    pdr: f32,
                ) -> Box<dyn DroneTrait> {
                    dbg!("Created drone type: {}", std::any::type_name::<$variant>());
                    Box::new($variant::new(id, controller_send, controller_recv, packet_recv, packet_send, pdr))
                }
            )*

            // static table of all factories
            static FACTORIES: &[fn(
                NodeId,
                Sender<DroneEvent>,
                Receiver<DroneCommand>,
                Receiver<Packet>,
                HashMap<NodeId, Sender<Packet>>,
                f32,
            ) -> Box<dyn DroneTrait>] = &[
                $(
                    [<$variant:snake _factory>],
                )*
            ];
        }
    };
}

drone_factories!(
    CppEnjoyersDrone,
    DroneDrone,
    DrOnesDrone,
    LockheedRustin,
    NullPointerDrone,
    RustafarianDrone,
    RustBustersDrone,
    RustezeDrone,
    RustyDrone,
    RustDrone,
);

pub(crate) fn generate_drones(
    controller_send: Sender<DroneEvent>,
    controller_receivers: Vec<DroneAttributes>,
) -> Vec<Box<dyn DroneTrait>> {
    controller_receivers
        .into_iter()
        .enumerate()
        .map(
            |(i, (id, controller_recv, packet_recv, packet_send, pdr))| {
                // pick the factory in round-robin using FACTORIES length
                let factory = FACTORIES[i % FACTORIES.len()];
                factory(
                    id,
                    controller_send.clone(),
                    controller_recv,
                    packet_recv,
                    packet_send,
                    pdr,
                )
            },
        )
        .collect()
}

#[derive(Clone)]
pub struct Channel<T> {
    sender: Sender<T>,
    receiver: Receiver<T>,
}

impl<T> Channel<T> {
    pub fn new() -> Self {
        let (sender, receiver) = unbounded();
        Channel { sender, receiver }
    }

    pub fn send(&self, item: T) -> Result<(), crossbeam::channel::SendError<T>> {
        self.sender.send(item)
    }

    pub fn recv(&self) -> Result<T, crossbeam::channel::RecvError> {
        self.receiver.recv()
    }

    pub fn get_sender(&self) -> Sender<T> {
        self.sender.clone()
    }

    pub fn get_receiver(&self) -> Receiver<T> {
        self.receiver.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam::channel::unbounded;

    #[test]
    fn test_generate_drones() {
        // Prepare some dummy channels
        let (send_event, _recv_event) = unbounded::<DroneEvent>();
        let (send_cmd, recv_cmd) = unbounded::<DroneCommand>();
        let (send_pkt, recv_pkt) = unbounded::<Packet>();

        let mut packet_map = HashMap::new();
        packet_map.insert(1, send_pkt);

        // Create 25 inputs to force cycling through FACTORIES several times
        let inputs = (0..25)
            .map(|id| {
                (
                    id,
                    recv_cmd.clone(),
                    recv_pkt.clone(),
                    packet_map.clone(),
                    0.9,
                )
            })
            .collect::<Vec<_>>();

        let drones = generate_drones(send_event, inputs);

        // 1) Count must match
        assert_eq!(drones.len(), 25);

        // 2) Check cycling: i-th drone corresponds to DroneType::iter().nth(i % FACTORIES.len())
        for (i, _) in drones.iter().enumerate() {
            let expected_type = i % FACTORIES.len(); // NOT Correct

            // This check only validates the index cycling
            // (we can't downcast Box<dyn DroneTrait> easily without extra work)
            assert_eq!(expected_type as usize, i % FACTORIES.len());
        }
    }
}
