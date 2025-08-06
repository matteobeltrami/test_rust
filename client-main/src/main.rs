use clap::{Arg, Command};
use client::Client;
use common::types::ClientType;
use wg_internal::network::NodeId;
use crossbeam::channel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("Drone Network Client")
        .version("1.0")
        .author("RustDoIt Team")
        .about("Client implementation for drone network communication")
        .arg(
            Arg::new("id")
                .short('i')
                .long("id")
                .value_name("CLIENT_ID")
                .help("Client ID (0-255)")
                .required(true),
        )
        .arg(
            Arg::new("name")
                .short('n')
                .long("name")
                .value_name("CLIENT_NAME")
                .help("Client name")
                .required(true),
        )
        .arg(
            Arg::new("type")
                .short('t')
                .long("type")
                .value_name("CLIENT_TYPE")
                .help("Client type: web or chat")
                .required(true),
        )
        .get_matches();

    let client_id: NodeId = matches
        .get_one::<String>("id")
        .unwrap()
        .parse()
        .expect("Client ID must be a number between 0-255");

    let client_name = matches.get_one::<String>("name").unwrap().clone();

    let client_type = match matches.get_one::<String>("type").unwrap().as_str() {
        "web" => ClientType::WebBrowser,
        "chat" => ClientType::ChatClient,
        _ => {
            eprintln!("Invalid client type. Use 'web' or 'chat'");
            std::process::exit(1);
        }
    };

    println!("🚀 Starting Drone Network Client");
    println!("   ID: {}", client_id);
    println!("   Name: {}", client_name);
    println!("   Type: {:?}", client_type);

    // Create packet channel for receiving messages from drones
    let (packet_sender, packet_receiver) = channel::unbounded();

    // Create client instance
    let client = Client::new(client_id, client_name, client_type);

    // In a real implementation, the network initializer would set up neighbors
    // For demo purposes, we'll simulate this
    println!("ℹ️  Note: In a real deployment, neighbors would be set up by the network initializer");
    
    // Start the client
    match client.start(packet_receiver).await {
        Ok(_) => {
            println!("✅ Client completed successfully");
        },
        Err(e) => {
            eprintln!("❌ Client error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam::channel;

    #[tokio::test]
    async fn test_client_creation() {
        let (_sender, receiver) = channel::unbounded();
        let client = Client::new(
            1, 
            "TestClient".to_string(), 
            ClientType::WebBrowser,
        );
        
        assert_eq!(client.get_id(), 1);
        assert_eq!(client.get_name(), "TestClient");
        assert!(matches!(client.get_type(), ClientType::WebBrowser));
    }

    #[tokio::test]
    async fn test_neighbor_management() {
        let (_sender, receiver) = channel::unbounded();
        let client = Client::new(
            1, 
            "TestClient".to_string(), 
            ClientType::ChatClient,
        );

        let (neighbor_sender, _) = channel::unbounded();
        client.add_neighbor(2, neighbor_sender).await;
        
        // In a real test, we'd verify the neighbor was added
        // This is a basic structure test
    }
}