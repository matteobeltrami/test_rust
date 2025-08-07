use clap::{Arg, Command};
use crossbeam::channel;
use std::collections::HashMap;
use std::thread;
use wg_internal::{
    config::Config,
    network::NodeId,
    packet::Packet,
    controller::{DroneCommand, DroneEvent},
};

mod errors;
mod parser;
mod utils;

use errors::ConfigError;
use parser::{Parse, Validate};

// Import drone implementations
use null_pointer_drone::Drone as NullPointerDrone;
use wg_2024_rust::Drone as WG2024Drone;
use d_r_o_n_e_drone::Drone as DroneZDrone;
use rusteze_drone::Drone as RustezeDrone;
use ap2024_unitn_cppenjoyers_drone::Drone as CppEnjoyersDrone;
use dr_ones::Drone as DrOnesDrone;
use rusty_drones::Drone as RustyDronesDrone;
use rustbusters_drone::Drone as RustbustersDrone;
use rustafarian_drone::Drone as RustafarianDrone;
use lockheedrustin_drone::Drone as LockheedRustinDrone;

// Import our server implementations
use text_server::TextServer;
use media_server::MediaServer;
use chat_server::CommunicationServer;

use wg_internal::drone::Drone;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let matches = Command::new("Network Initializer")
        .version("1.0")
        .about("Initializes the drone network with servers and clients")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("CONFIG_FILE")
                .help("Path to the network configuration TOML file")
                .required(true),
        )
        .arg(
            Arg::new("text-servers")
                .long("text-servers")
                .value_name("COUNT")
                .help("Number of text servers to spawn")
                .default_value("1"),
        )
        .arg(
            Arg::new("media-servers")
                .long("media-servers")
                .value_name("COUNT")
                .help("Number of media servers to spawn")
                .default_value("1"),
        )
        .arg(
            Arg::new("chat-servers")
                .long("chat-servers")
                .value_name("COUNT")
                .help("Number of communication servers to spawn")
                .default_value("1"),
        )
        .get_matches();

    let config_path = matches.get_one::<String>("config").unwrap();
    let text_servers_count: usize = matches.get_one::<String>("text-servers").unwrap().parse()?;
    let media_servers_count: usize = matches.get_one::<String>("media-servers").unwrap().parse()?;
    let chat_servers_count: usize = matches.get_one::<String>("chat-servers").unwrap().parse()?;

    println!("🚀 Starting Network Initializer");
    println!("   Config file: {}", config_path);
    println!("   Text servers: {}", text_servers_count);
    println!("   Media servers: {}", media_servers_count);
    println!("   Chat servers: {}", chat_servers_count);

    // Parse and validate configuration
    let config = Config::parse_config(config_path)?;
    config.validate_config()?;

    println!("✅ Configuration validated successfully");
    println!("   Drones: {}", config.drone.len());
    println!("   Clients: {}", config.client.len());
    println!("   Servers: {}", config.server.len());

    // Initialize the network
    let network = NetworkInitializer::new(config);
    network.initialize_network(text_servers_count, media_servers_count, chat_servers_count)?;

    println!("✅ Network initialization completed");
    println!("🔄 Network is now running - press Ctrl+C to stop");

    // Keep the main thread alive
    loop {
        thread::sleep(std::time::Duration::from_secs(1));
    }
}

struct NetworkInitializer {
    config: Config,
}

impl NetworkInitializer {
    fn new(config: Config) -> Self {
        Self { config }
    }

    fn initialize_network(
        &self,
        text_servers_count: usize,
        media_servers_count: usize,
        chat_servers_count: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create channels for all nodes
        let mut node_senders: HashMap<NodeId, channel::Sender<Packet>> = HashMap::new();
        let mut node_receivers: HashMap<NodeId, channel::Receiver<Packet>> = HashMap::new();

        // Create controller channels
        let mut controller_senders: HashMap<NodeId, channel::Sender<DroneCommand>> = HashMap::new();
        let mut controller_receivers: HashMap<NodeId, channel::Receiver<DroneCommand>> = HashMap::new();
        let mut event_senders: HashMap<NodeId, channel::Sender<DroneEvent>> = HashMap::new();
        let mut event_receivers: HashMap<NodeId, channel::Receiver<DroneEvent>> = HashMap::new();

        // Create packet channels for all nodes
        for drone in &self.config.drone {
            let (sender, receiver) = channel::unbounded();
            node_senders.insert(drone.id, sender);
            node_receivers.insert(drone.id, receiver);

            let (cmd_sender, cmd_receiver) = channel::unbounded();
            controller_senders.insert(drone.id, cmd_sender);
            controller_receivers.insert(drone.id, cmd_receiver);

            let (event_sender, event_receiver) = channel::unbounded();
            event_senders.insert(drone.id, event_sender);
            event_receivers.insert(drone.id, event_receiver);
        }

        for client in &self.config.client {
            let (sender, receiver) = channel::unbounded();
            node_senders.insert(client.id, sender);
            node_receivers.insert(client.id, receiver);
        }

        for server in &self.config.server {
            let (sender, receiver) = channel::unbounded();
            node_senders.insert(server.id, sender);
            node_receivers.insert(server.id, receiver);
        }

        // Assign server IDs starting from the end of the configured server range
        let mut next_server_id = self.config.server.iter().map(|s| s.id).max().unwrap_or(200) + 1;

        // Create additional server channels
        for _ in 0..(text_servers_count + media_servers_count + chat_servers_count) {
            let (sender, receiver) = channel::unbounded();
            node_senders.insert(next_server_id, sender);
            node_receivers.insert(next_server_id, receiver);

            let (event_sender, event_receiver) = channel::unbounded();
            event_senders.insert(next_server_id, event_sender);
            event_receivers.insert(next_server_id, event_receiver);

            let (cmd_sender, cmd_receiver) = channel::unbounded();
            controller_senders.insert(next_server_id, cmd_sender);
            controller_receivers.insert(next_server_id, cmd_receiver);

            next_server_id += 1;
        }

        println!("📡 Setting up communication channels...");

        // Start drones with distributed implementations
        self.start_drones(&mut node_senders, node_receivers, controller_receivers, event_senders)?;

        // Start servers
        let mut server_id = self.config.server.iter().map(|s| s.id).max().unwrap_or(200) + 1;
        
        // Start text servers
        for i in 0..text_servers_count {
            println!("🗃️  Starting Text Server {}", i + 1);
            self.start_text_server(
                server_id,
                &mut node_senders,
                node_receivers.remove(&server_id).unwrap(),
                controller_receivers.remove(&server_id).unwrap(),
                event_senders.remove(&server_id).unwrap(),
            )?;
            server_id += 1;
        }

        // Start media servers  
        for i in 0..media_servers_count {
            println!("🖼️  Starting Media Server {}", i + 1);
            self.start_media_server(
                server_id,
                &mut node_senders,
                node_receivers.remove(&server_id).unwrap(),
                controller_receivers.remove(&server_id).unwrap(),
                event_senders.remove(&server_id).unwrap(),
            )?;
            server_id += 1;
        }

        // Start communication servers
        for i in 0..chat_servers_count {
            println!("💬 Starting Communication Server {}", i + 1);
            self.start_communication_server(
                server_id,
                &mut node_senders,
                node_receivers.remove(&server_id).unwrap(),
                controller_receivers.remove(&server_id).unwrap(),
                event_senders.remove(&server_id).unwrap(),
            )?;
            server_id += 1;
        }

        println!("✅ All nodes started successfully");

        Ok(())
    }

    fn start_drones(
        &self,
        node_senders: &mut HashMap<NodeId, channel::Sender<Packet>>,
        mut node_receivers: HashMap<NodeId, channel::Receiver<Packet>>,
        mut controller_receivers: HashMap<NodeId, channel::Receiver<DroneCommand>>,
        mut event_senders: HashMap<NodeId, channel::Sender<DroneEvent>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let drone_implementations = [
            "null-pointer", "wg2024-rust", "d-r-o-n-e", "rusteze", "cpp-enjoyers",
            "dr-ones", "rusty-drones", "rustbusters", "rustafarian", "lockheed-rustin"
        ];

        for (index, drone_config) in self.config.drone.iter().enumerate() {
            println!("🚁 Starting Drone {} with implementation: {}", 
                drone_config.id, 
                drone_implementations[index % drone_implementations.len()]
            );

            // Create neighbor senders for this drone
            let mut neighbor_senders = HashMap::new();
            for &neighbor_id in &drone_config.connected_node_ids {
                if let Some(sender) = node_senders.get(&neighbor_id) {
                    neighbor_senders.insert(neighbor_id, sender.clone());
                }
            }

            let drone_id = drone_config.id;
            let pdr = drone_config.pdr;
            let packet_receiver = node_receivers.remove(&drone_id).unwrap();
            let controller_receiver = controller_receivers.remove(&drone_id).unwrap();
            let event_sender = event_senders.remove(&drone_id).unwrap();

            // Select drone implementation based on round-robin
            let implementation_index = index % drone_implementations.len();
            
            thread::spawn(move || {
                let mut drone = match implementation_index {
                    0 => Box::new(NullPointerDrone::new(
                        drone_id, event_sender, controller_receiver, packet_receiver, neighbor_senders, pdr
                    )) as Box<dyn Drone>,
                    1 => Box::new(WG2024Drone::new(
                        drone_id, event_sender, controller_receiver, packet_receiver, neighbor_senders, pdr
                    )) as Box<dyn Drone>,
                    2 => Box::new(DroneZDrone::new(
                        drone_id, event_sender, controller_receiver, packet_receiver, neighbor_senders, pdr
                    )) as Box<dyn Drone>,
                    3 => Box::new(RustezeDrone::new(
                        drone_id, event_sender, controller_receiver, packet_receiver, neighbor_senders, pdr
                    )) as Box<dyn Drone>,
                    4 => Box::new(CppEnjoyersDrone::new(
                        drone_id, event_sender, controller_receiver, packet_receiver, neighbor_senders, pdr
                    )) as Box<dyn Drone>,
                    5 => Box::new(DrOnesDrone::new(
                        drone_id, event_sender, controller_receiver, packet_receiver, neighbor_senders, pdr
                    )) as Box<dyn Drone>,
                    6 => Box::new(RustyDronesDrone::new(
                        drone_id, event_sender, controller_receiver, packet_receiver, neighbor_senders, pdr
                    )) as Box<dyn Drone>,
                    7 => Box::new(RustbustersDrone::new(
                        drone_id, event_sender, controller_receiver, packet_receiver, neighbor_senders, pdr
                    )) as Box<dyn Drone>,
                    8 => Box::new(RustafarianDrone::new(
                        drone_id, event_sender, controller_receiver, packet_receiver, neighbor_senders, pdr
                    )) as Box<dyn Drone>,
                    9 => Box::new(LockheedRustinDrone::new(
                        drone_id, event_sender, controller_receiver, packet_receiver, neighbor_senders, pdr
                    )) as Box<dyn Drone>,
                    _ => unreachable!(),
                };

                drone.run();
            });
        }

        Ok(())
    }

    fn start_text_server(
        &self,
        server_id: NodeId,
        node_senders: &mut HashMap<NodeId, channel::Sender<Packet>>,
        packet_receiver: channel::Receiver<Packet>,
        controller_receiver: channel::Receiver<DroneCommand>,
        event_sender: channel::Sender<DroneEvent>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Find drone neighbors (connect to first 2 available drones)
        let drone_neighbors: Vec<NodeId> = self.config.drone.iter()
            .take(2)
            .map(|d| d.id)
            .collect();

        let mut server = TextServer::new(server_id);
        server.set_channels(packet_receiver, event_sender, controller_receiver);

        // Connect to drone neighbors
        for &drone_id in &drone_neighbors {
            if let Some(sender) = node_senders.get(&drone_id) {
                let server_ref = &server;
                tokio::spawn(async move {
                    server_ref.add_neighbor(drone_id, sender.clone()).await;
                });
            }
        }

        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Err(e) = server.run().await {
                    eprintln!("Text Server {} error: {}", server_id, e);
                }
            });
        });

        Ok(())
    }

    fn start_media_server(
        &self,
        server_id: NodeId,
        node_senders: &mut HashMap<NodeId, channel::Sender<Packet>>,
        packet_receiver: channel::Receiver<Packet>,
        controller_receiver: channel::Receiver<DroneCommand>,
        event_sender: channel::Sender<DroneEvent>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Find drone neighbors (connect to first 2 available drones)
        let drone_neighbors: Vec<NodeId> = self.config.drone.iter()
            .skip(2) // Use different drones than text server
            .take(2)
            .map(|d| d.id)
            .collect();

        let mut server = MediaServer::new(server_id);
        server.set_channels(packet_receiver, event_sender, controller_receiver);

        // Connect to drone neighbors
        for &drone_id in &drone_neighbors {
            if let Some(sender) = node_senders.get(&drone_id) {
                let server_ref = &server;
                tokio::spawn(async move {
                    server_ref.add_neighbor(drone_id, sender.clone()).await;
                });
            }
        }

        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Err(e) = server.run().await {
                    eprintln!("Media Server {} error: {}", server_id, e);
                }
            });
        });

        Ok(())
    }

    fn start_communication_server(
        &self,
        server_id: NodeId,
        node_senders: &mut HashMap<NodeId, channel::Sender<Packet>>,
        packet_receiver: channel::Receiver<Packet>,
        controller_receiver: channel::Receiver<DroneCommand>,
        event_sender: channel::Sender<DroneEvent>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Find drone neighbors (connect to first 2 available drones, different from others)
        let drone_neighbors: Vec<NodeId> = self.config.drone.iter()
            .skip(4) // Use different drones
            .take(2)
            .map(|d| d.id)
            .collect();

        let mut server = CommunicationServer::new(server_id);
        server.set_channels(packet_receiver, event_sender, controller_receiver);

        // Connect to drone neighbors
        for &drone_id in &drone_neighbors {
            if let Some(sender) = node_senders.get(&drone_id) {
                let server_ref = &server;
                tokio::spawn(async move {
                    server_ref.add_neighbor(drone_id, sender.clone()).await;
                });
            }
        }

        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Err(e) = server.run().await {
                    eprintln!("Communication Server {} error: {}", server_id, e);
                }
            });
        });

        Ok(())
    }
}