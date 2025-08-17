// TODO: togliere
#![allow(dead_code)]
#![allow(unused_variables)]
use crate::parser::{Parse, Validate};
use crate::utils::Channel;
use common::network::Network;
use common::types::{NodeCommand, NodeEvent};
use crossbeam::channel::{Receiver, Sender};
use std::collections::HashMap;
use std::thread::JoinHandle;
use wg_internal::config::Config;
use wg_internal::controller::{DroneCommand, DroneEvent};
use wg_internal::network::NodeId;
use wg_internal::packet::Packet;

pub struct Initialized;
pub struct Uninitialized;
pub struct Running;

pub struct NetworkInitializer<State = Uninitialized> {
    communications_channels: HashMap<NodeId, Channel<Packet>>,
    command_channels: HashMap<NodeId, Channel<DroneCommand>>,
    config: Config,
    state: std::marker::PhantomData<State>,
    node_event: Channel<DroneEvent>,
    network_view: Option<Network>,
    clients: HashMap<NodeId, (Sender<NodeCommand>, Receiver<NodeEvent>)>,
    servers: HashMap<NodeId, (Sender<NodeCommand>, Receiver<NodeEvent>)>,
    drones: HashMap<NodeId, (Sender<DroneCommand>, Receiver<DroneEvent>)>,
    node_handles: HashMap<NodeId, JoinHandle<()>>
}

impl NetworkInitializer<Uninitialized> {
    pub fn new(config_path: &str) -> Self {
        let config = Config::parse_config(config_path).expect("Failed to parse config");
        if let Err(e) = config.validate_config() {
            panic!("Configuration validation failed: {}", e);
        }
        Self {
            communications_channels: HashMap::new(),
            command_channels: HashMap::new(),
            config,
            state: std::marker::PhantomData,
            node_event: Channel::new(),
            network_view: None,
            clients: HashMap::new(),
            servers: HashMap::new(),
            drones: HashMap::new(),
            node_handles: HashMap::new()
        }
    }

    pub fn initialize(mut self) -> NetworkInitializer<Initialized> {
        self.initialize_drones();
        self.initilize_clients();
        self.initialize_servers();
        NetworkInitializer::<Initialized>::new(self)
    }

    fn initialize_drones(&mut self) {
        unimplemented!()
    }

    fn initilize_clients(&mut self) {
        unimplemented!()
    }

    fn initialize_servers(&mut self) {
        unimplemented!()
    }
}

impl NetworkInitializer<Initialized> {
    fn new(initializer: NetworkInitializer<Uninitialized>) -> Self {
        unimplemented!()
    }

    pub fn start_simulation(&mut self) -> NetworkInitializer<Running> {
        unimplemented!()
    }
}


impl NetworkInitializer<Running> {
    fn new(initializer: NetworkInitializer<Initialized>) -> Self {
        unimplemented!()
    }

    pub fn stop_simulation(&self) {
        unimplemented!()
    }

    pub fn get_drones(&self) -> HashMap<NodeId, (Sender<DroneCommand>, Receiver<DroneEvent>)> {
        unimplemented!()
    }

    pub fn get_clients(&self) -> HashMap<NodeId, (Sender<NodeCommand>, Receiver<NodeEvent>)> {
        unimplemented!()
    }

    pub fn get_servers(&self) -> HashMap<NodeId, (Sender<NodeCommand>, Receiver<NodeEvent>)> {
        unimplemented!()
    }

    fn get_network_view(&self) -> Network {
        unimplemented!()
    }
}