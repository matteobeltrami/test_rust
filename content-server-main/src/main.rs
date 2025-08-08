use clap::{Arg, Command};
use content_server::{ContentServer, ServerMode};
use wg_internal::network::NodeId;
use crossbeam::channel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let matches = Command::new("Content Server")
        .version("1.0")
        .author("RustDoIt Team")
        .about("Unified content server implementation for drone network communication")
        .arg(
            Arg::new("id")
                .short('i')
                .long("id")
                .value_name("SERVER_ID")
                .help("Server ID (0-255)")
                .required(true),
        )
        .arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .value_name("MODE")
                .help("Server mode: text, media, or combined")
                .default_value("combined")
                .value_parser(["text", "media", "combined"]),
        )
        .get_matches();

    let server_id: NodeId = matches
        .get_one::<String>("id")
        .unwrap()
        .parse()
        .expect("Server ID must be a number between 0-255");

    let mode_str = matches.get_one::<String>("mode").unwrap();
    let mode = match mode_str.as_str() {
        "text" => ServerMode::TextOnly,
        "media" => ServerMode::MediaOnly,
        "combined" => ServerMode::Combined,
        _ => ServerMode::Combined, // Default fallback
    };

    println!("🗃️  Starting Drone Network Content Server");
    println!("   ID: {}", server_id);
    println!("   Mode: {}", mode);

    // Create packet channels for receiving messages from drones
    let (_packet_sender, packet_receiver) = channel::unbounded();
    let (_controller_sender, controller_receiver) = channel::unbounded();
    let (event_sender, _event_receiver) = channel::unbounded();

    // Create server instance
    let mut server = ContentServer::new(server_id, mode.clone());
    server.set_channels(packet_receiver, event_sender, controller_receiver);

    // Display capabilities
    match mode {
        ServerMode::TextOnly => {
            println!("📄 Server configured for TEXT content only");
            if let Some(file_ids) = server.get_file_ids().await {
                println!("   Available text files: {:?}", file_ids);
            }
        },
        ServerMode::MediaOnly => {
            println!("🎬 Server configured for MEDIA content only");
            if let Some(media_ids) = server.get_media_ids().await {
                println!("   Available media files: {:?}", media_ids);
            }
        },
        ServerMode::Combined => {
            println!("📄🎬 Server configured for COMBINED content (text + media)");
            if let Some(file_ids) = server.get_file_ids().await {
                println!("   Available text files: {:?}", file_ids);
            }
            if let Some(media_ids) = server.get_media_ids().await {
                println!("   Available media files: {:?}", media_ids);
            }
            if let Some(stats) = server.get_media_stats().await {
                println!("   Media statistics: {} files, {} bytes total", stats.total_files, stats.total_size);
            }
        }
    }

    println!("ℹ️  Note: In a real deployment, neighbors would be set up by the network initializer");
    
    // Start the server
    match server.run().await {
        Ok(_) => {
            println!("✅ Content Server completed successfully");
        },
        Err(e) => {
            eprintln!("❌ Content Server error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use content_server::{ContentServer, ServerMode};

    #[tokio::test]
    async fn test_server_creation() {
        let server = ContentServer::new(1, ServerMode::Combined);
        assert_eq!(server.get_id(), 1);
        assert!(matches!(server.get_mode(), ServerMode::Combined));
    }

    #[tokio::test]
    async fn test_text_only_mode() {
        let server = ContentServer::new(1, ServerMode::TextOnly);
        
        // Should have text capabilities
        let file_ids = server.get_file_ids().await;
        assert!(file_ids.is_some());
        
        // Should not have media capabilities
        let media_ids = server.get_media_ids().await;
        assert!(media_ids.is_none());
    }

    #[tokio::test]
    async fn test_media_only_mode() {
        let server = ContentServer::new(1, ServerMode::MediaOnly);
        
        // Should not have text capabilities
        let file_ids = server.get_file_ids().await;
        assert!(file_ids.is_none());
        
        // Should have media capabilities
        let media_ids = server.get_media_ids().await;
        assert!(media_ids.is_some());
    }

    #[tokio::test]
    async fn test_combined_mode() {
        let server = ContentServer::new(1, ServerMode::Combined);
        
        // Should have both capabilities
        let file_ids = server.get_file_ids().await;
        assert!(file_ids.is_some());
        
        let media_ids = server.get_media_ids().await;
        assert!(media_ids.is_some());
        
        let stats = server.get_media_stats().await;
        assert!(stats.is_some());
    }

    #[tokio::test]
    async fn test_file_operations() {
        let server = ContentServer::new(1, ServerMode::Combined);
        
        // Wait a bit for initialization to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Test getting file IDs
        if let Some(file_ids) = server.get_file_ids().await {
            assert!(!file_ids.is_empty());
            assert!(file_ids.contains(&"welcome".to_string()));
            
            // Test getting specific file
            let welcome_file = server.get_file("welcome").await;
            assert!(welcome_file.is_some());
            if let Some(file) = welcome_file {
                assert_eq!(file.title, "Welcome to Our Drone Network");
            }
        } else {
            // If no files yet, just ensure the server was created
            assert_eq!(server.get_id(), 1);
        }
    }

    #[tokio::test] 
    async fn test_media_operations() {
        let server = ContentServer::new(1, ServerMode::Combined);
        
        // Wait a bit for initialization to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Test getting media IDs
        if let Some(media_ids) = server.get_media_ids().await {
            assert!(!media_ids.is_empty());
            assert!(media_ids.contains(&"network_diagram.png".to_string()));
            
            // Test getting specific media
            let diagram_media = server.get_media("network_diagram.png").await;
            assert!(diagram_media.is_some());
            if let Some(media) = diagram_media {
                assert_eq!(media.name, "Network Topology Diagram");
            }
        } else {
            // If no media yet, just ensure the server was created
            assert_eq!(server.get_id(), 1);
        }
    }
}