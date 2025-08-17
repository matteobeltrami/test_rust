#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use ap2024_unitn_cppenjoyers_drone::CppEnjoyersDrone;
use common::types::NodeCommand;
use crossbeam::channel::{Receiver, Sender, unbounded};
use d_r_o_n_e_drone::MyDrone as DroneDrone;
use dr_ones::Drone as DrOnesDrone;
use lockheedrustin_drone::LockheedRustin;
use null_pointer_drone::MyDrone as NullPointerDrone;
use rustafarian_drone::RustafarianDrone;
use rustbusters_drone::RustBustersDrone;
use rusteze_drone::RustezeDrone;
use rusty_drones::RustyDrone;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use wg_internal::config::{Client, Drone, Server};
use wg_internal::controller::DroneCommand;
use wg_internal::network::NodeId;

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

#[derive(Debug, EnumIter)]
pub enum DroneType {
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
