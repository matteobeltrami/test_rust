use clap::{Arg, Command};
use text_server::TextServer;
use wg_internal::network::NodeId;
use crossbeam::channel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let matches = Command::new("Text Server")
        .version("1.0")
        .author("RustDoIt Team")
        .about("Text server implementation for drone network communication")
        .arg(
            Arg::new("id")
                .short('i')
                .long("id")
                .value_name("SERVER_ID")
                .help("Server ID (0-255)")
                .required(true),
        )
        .get_matches();

    let server_id: NodeId = matches
        .get_one::<String>("id")
        .unwrap()
        .parse()
        .expect("Server ID must be a number between 0-255");

    println!("🗃️  Starting Drone Network Text Server");
    println!("   ID: {}", server_id);

    // Create packet channel for receiving messages from drones
    let (packet_sender, packet_receiver) = channel::unbounded();
    let (controller_sender, controller_receiver) = channel::unbounded();
    let (event_sender, event_receiver) = channel::unbounded();

    // Create server instance
    let mut server = TextServer::new(server_id);
    server.set_channels(packet_receiver, event_sender, controller_receiver);

    println!("ℹ️  Note: In a real deployment, neighbors would be set up by the network initializer");
    
    // Start the server
    match server.run().await {
        Ok(_) => {
            println!("✅ Text Server completed successfully");
        },
        Err(e) => {
            eprintln!("❌ Text Server error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let server = TextServer::new(1);
        assert_eq!(server.get_id(), 1);
    }

    #[tokio::test]
    async fn test_file_operations() {
        let server = TextServer::new(1);
        
        // Test getting file IDs
        let file_ids = server.get_file_ids().await;
        assert!(!file_ids.is_empty());
        assert!(file_ids.contains(&"welcome".to_string()));
        
        // Test getting specific file
        let welcome_file = server.get_file("welcome").await;
        assert!(welcome_file.is_some());
        assert_eq!(welcome_file.unwrap().title, "Welcome to Our Drone Network");
    }
}