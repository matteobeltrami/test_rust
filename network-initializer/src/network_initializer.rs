// TODO: togliere
#![allow(dead_code)]
#![allow(unused_variables)]
use crate::parser::{Parse, Validate};
use crate::utils::{Channel, generate_drones};
use client::chat_client::ChatClient;
use client::web_browser::WebBrowser;
use common::Processor;
use common::network::Network;
use common::types::{Command, Event, NodeType as CommonNodeType};
use crossbeam::channel::{Receiver, Sender};
use server::{ChatServer, MediaServer, TextServer};
use std::collections::HashMap;
use std::thread::JoinHandle;
use wg_internal::config::Config;
use wg_internal::controller::{DroneCommand, DroneEvent};
use wg_internal::drone::Drone;
use wg_internal::network::NodeId;
use wg_internal::packet::{NodeType, Packet};

pub struct Uninitialized;
pub struct Initialized;
pub struct Running;

pub struct NetworkInitializer<State = Uninitialized> {
    // node_id, sender to that node
    communications_channels: HashMap<NodeId, Channel<Packet>>,
    // each drone has his command receiver, controller needs senders to send commands
    drone_command_channels: HashMap<NodeId, Sender<DroneCommand>>,
    // each node has his command receiver, controller needs senders to send commands
    node_command_channels: HashMap<NodeId, (CommonNodeType, Sender<Box<dyn Command>>)>,
    // controller receives events from drones
    drone_event_channel: Channel<DroneEvent>,
    // controller receives events from nodes
    node_event_channel: Channel<Box<dyn Event>>,
    total_nodes: usize,
    pub(crate) config: Config,
    // do not exists
    state: std::marker::PhantomData<State>,
    // TODO: create topology based on config
    network_view: Option<Network>,

    // these are needed to NetworkInitializer<Running> to run each node
    initialized_clients: Vec<Box<dyn Processor + Send>>,
    initialized_servers: Vec<Box<dyn Processor + Send>>,
    initialized_drones: Vec<Box<dyn Drone>>,

    // to keep track of threads and join them at the end
    node_handles: Vec<JoinHandle<()>>,
}

impl NetworkInitializer<Uninitialized> {
    /// # Panics
    /// Panics it cannot parse the config or the config is not a valid config
    #[must_use]
    pub fn new(config_path: &str) -> Self {
        let config = Config::parse_config(config_path).expect("Failed to parse config");
        config.validate_config().expect("Failed to validate config");
        Self {
            communications_channels: HashMap::new(),
            drone_command_channels: HashMap::new(),
            node_command_channels: HashMap::new(),
            drone_event_channel: Channel::new(),
            node_event_channel: Channel::new(),
            total_nodes: config.drone.len() + config.client.len() + config.server.len(),
            config,
            // do not exists
            state: std::marker::PhantomData,
            network_view: None,
            initialized_clients: Vec::new(),
            initialized_servers: Vec::new(),
            initialized_drones: Vec::new(),
            node_handles: Vec::new(),
        }
    }

    #[must_use]
    pub fn initialize(mut self) -> NetworkInitializer<Initialized> {
        self.initialize_drones();
        self.initialize_clients();
        self.initialize_servers();
        self.inizialize_network_view();
        NetworkInitializer::<Initialized>::new(self)
    }

    fn initialize_drones(&mut self) {
        let mut drones_attributes = Vec::new();
        // first create all channelsWW
        for d in &self.config.drone {
            self.communications_channels.insert(d.id, Channel::new());
        }

        // then this
        for d in &self.config.drone {
            let command_channel = Channel::new();
            let mut neighbors = HashMap::new();
            for id in &d.connected_node_ids {
                if let Some(channel) = self.communications_channels.get(id) {
                    neighbors.insert(*id, channel.get_sender());
                }
            }
            // initializing receiver channel of the drone
            if let Some(packet_receiver) = self.communications_channels.get(&d.id) {
                drones_attributes.push((
                    d.id,
                    command_channel.get_receiver(),
                    packet_receiver.get_receiver(),
                    neighbors,
                    d.pdr,
                ));
            }
            self.drone_command_channels
                .insert(d.id, command_channel.get_sender());
        }

        self.initialized_drones =
            generate_drones(&self.drone_event_channel.sender, drones_attributes);
    }

    fn initialize_clients(&mut self) {
        for (idx, c) in self.config.client.iter().enumerate() {
            // create neighbors
            let mut neighbors = HashMap::new();
            c.connected_drone_ids.iter().for_each(|id| {
                if let Some(channel) = self.communications_channels.get(id) {
                    neighbors.insert(*id, channel.get_sender());
                }
            });
            //create the channels
            let packet_channel = Channel::new();
            let command_channel = Channel::new();
            #[allow(clippy::needless_late_init)]
            let client: Box<dyn Processor>;
            let node_type: CommonNodeType;
            // instantiate client
            if idx == 0 {
                client = Box::new(WebBrowser::new(
                    c.id,
                    neighbors,
                    packet_channel.get_receiver(),
                    command_channel.get_receiver(),
                    self.node_event_channel.get_sender(),
                ));
                node_type = CommonNodeType::WebBrowser;
            } else {
                client = Box::new(ChatClient::new(
                    c.id,
                    neighbors,
                    packet_channel.get_receiver(),
                    command_channel.get_receiver(),
                    self.node_event_channel.get_sender(),
                ));
                node_type = CommonNodeType::ChatClient;
            }

            // save the channels
            self.communications_channels.insert(c.id, packet_channel);
            self.node_command_channels
                .insert(c.id, (node_type, command_channel.get_sender()));

            // save the client
            self.initialized_clients.push(client);
        }
    }

    fn initialize_servers(&mut self) {
        for (i, s) in self.config.server.iter().enumerate() {
            let server: Box<dyn Processor>;
            let packet_channel = Channel::new();
            let mut neighbors = HashMap::new();
            s.connected_drone_ids.iter().for_each(|id| {
                if let Some(channel) = self.communications_channels.get(id) {
                    neighbors.insert(*id, channel.get_sender());
                }
            });
            let node_type: CommonNodeType;

            let command_channel = Channel::new();

            match i % 3 {
                0 => {
                    server = Box::new(TextServer::new(
                        s.id,
                        neighbors.clone(),
                        packet_channel.get_receiver(),
                        command_channel.get_receiver(),
                        self.node_event_channel.get_sender(),
                    ));
                    node_type = CommonNodeType::TextServer;
                }
                1 => {
                    server = Box::new(MediaServer::new(
                        s.id,
                        neighbors.clone(),
                        packet_channel.get_receiver(),
                        command_channel.get_receiver(),
                        self.node_event_channel.get_sender(),
                    ));
                    node_type = CommonNodeType::MediaServer;
                }
                2 => {
                    server = Box::new(ChatServer::new(
                        s.id,
                        neighbors.clone(),
                        packet_channel.get_receiver(),
                        command_channel.get_receiver(),
                        self.node_event_channel.get_sender(),
                    ));
                    node_type = CommonNodeType::ChatServer;
                }
                _ => unreachable!(),
            }
            self.communications_channels.insert(s.id, packet_channel);
            self.node_command_channels
                .insert(s.id, (node_type, command_channel.get_sender()));
            self.initialized_servers.push(server);
        }
    }

    fn inizialize_network_view(&mut self) {
        let mut network = Network::default();
        for d in &self.config.drone {
            network.add_node_controller_view(d.id, NodeType::Drone, &d.connected_node_ids);
        }
        for c in &self.config.client {
            network.add_node_controller_view(c.id, NodeType::Client, &c.connected_drone_ids);
        }

        for s in &self.config.server {
            network.add_node_controller_view(s.id, NodeType::Server, &s.connected_drone_ids);
        }
        self.network_view = Some(network);
    }
}

impl NetworkInitializer<Initialized> {
    fn new(initializer: NetworkInitializer<Uninitialized>) -> Self {
        Self {
            communications_channels: initializer.communications_channels,
            drone_command_channels: initializer.drone_command_channels,
            node_command_channels: initializer.node_command_channels,
            drone_event_channel: initializer.drone_event_channel,
            node_event_channel: initializer.node_event_channel,
            total_nodes: initializer.total_nodes,
            config: initializer.config,
            state: std::marker::PhantomData,
            network_view: initializer.network_view,
            initialized_clients: initializer.initialized_clients,
            initialized_servers: initializer.initialized_servers,
            initialized_drones: initializer.initialized_drones,
            node_handles: Vec::new(),
        }
    }

    #[must_use]
    pub fn start_simulation(mut self) -> NetworkInitializer<Running> {
        for mut drone in self.initialized_drones.drain(..) {
            let handle = std::thread::spawn(move || {
                drone.run();
            });
            self.node_handles.push(handle);
        }
        for mut client in self.initialized_clients.drain(..) {
            let handle = std::thread::spawn(move || {
                client.run();
            });
            self.node_handles.push(handle);
        }
        for mut server in self.initialized_servers.drain(..) {
            let handle = std::thread::spawn(move || {
                server.run();
            });
            self.node_handles.push(handle);
        }
        NetworkInitializer::<Running>::new(self)
    }
}

impl NetworkInitializer<Running> {
    fn new(initializer: NetworkInitializer<Initialized>) -> Self {
        assert!(
            initializer.initialized_drones.is_empty(),
            "Drones should have been moved"
        );
        assert!(
            initializer.initialized_clients.is_empty(),
            "Clients should have been moved"
        );
        assert!(
            initializer.initialized_servers.is_empty(),
            "Servers should have been moved"
        );
        assert!(
            initializer.node_handles.len() == initializer.total_nodes,
            "All nodes should have been started"
        );
        assert!(
            initializer.network_view.is_some(),
            "Network should be initialized"
        );

        Self {
            communications_channels: initializer.communications_channels,
            drone_command_channels: initializer.drone_command_channels,
            node_command_channels: initializer.node_command_channels,
            drone_event_channel: initializer.drone_event_channel,
            node_event_channel: initializer.node_event_channel,
            total_nodes: initializer.total_nodes,
            config: initializer.config,
            state: std::marker::PhantomData,
            network_view: initializer.network_view,
            initialized_clients: initializer.initialized_clients,
            initialized_servers: initializer.initialized_servers,
            initialized_drones: initializer.initialized_drones,
            node_handles: initializer.node_handles,
        }
    }

    /// # Panics
    /// Panics if it cannot join handle
    pub fn stop_simulation(&mut self) {
        for (_, channel) in self.drone_command_channels.drain() {
            let _ = channel.send(DroneCommand::Crash);
            drop(channel);
        }
        for (_, (_, channel)) in self.node_command_channels.drain() {
            let _ = channel.send(Box::new(common::types::NodeCommand::Shutdown));
            drop(channel);
        }
        for (_, channel) in self.communications_channels.drain() {
            drop(channel.get_sender());
        }
        
        for handle in self.node_handles.drain(..) {
            match handle.join() {
                Ok(_) => {
                    println!("Terminated a node thread successfully");
                }
                Err(e) => {
                    eprintln!("Failed to join a node thread: {:?}", e);
                }
            }
        }
    }

    #[must_use]
    pub fn get_nodes_event_receiver(&self) -> Receiver<Box<dyn Event>> {
        self.node_event_channel.get_receiver()
    }

    #[must_use]
    pub fn get_drones_event_receiver(&self) -> Receiver<DroneEvent> {
        self.drone_event_channel.get_receiver()
    }

    #[must_use]
    pub fn get_drones(&self) -> HashMap<NodeId, (f32, Sender<DroneCommand>)> {
        let mut map = HashMap::new();
        for d in &self.config.drone {
            if let Some(channel) = self.drone_command_channels.get(&d.id) {
                map.insert(d.id, (d.pdr, channel.clone()));
            }
        }
        map
    }

    #[must_use]
    pub fn get_clients(&self) -> HashMap<NodeId, (CommonNodeType, Sender<Box<dyn Command>>)> {
        let mut map = HashMap::new();
        for c in &self.config.client {
            if let Some((node_type, channel)) = self.node_command_channels.get(&c.id) {
                map.insert(c.id, (*node_type, channel.clone()));
            }
        }
        map
    }

    #[must_use]
    pub fn get_servers(&self) -> HashMap<NodeId, (CommonNodeType, Sender<Box<dyn Command>>)> {
        let mut map = HashMap::new();
        for s in &self.config.server {
            if let Some((node_type, channel)) = self.node_command_channels.get(&s.id) {
                map.insert(s.id, (*node_type, channel.clone()));
            }
        }
        map
    }

    #[must_use]
    /// # Panics
    /// Panisce if not Initialized
    pub fn get_network_view(&self) -> Network {
        self.network_view.clone().expect("Network not Initialized")
    }

    #[must_use]
    fn get_comms_channels(&self) -> &HashMap<NodeId, Channel<Packet>> {
        &self.communications_channels
    }
}
