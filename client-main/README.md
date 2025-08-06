# Drone Network Client Implementation

A comprehensive Rust client implementation for the Advanced Programming 2024/2025 drone network project. This client supports both Web Browser and Chat functionality according to the AP-protocol specification.

## 🚀 Features

- **Web Browser Client**: Browse and retrieve files from Text and Media servers
- **Chat Client**: Register with Communication servers and exchange messages with other clients
- **Network Discovery**: Automatic network topology discovery using flood protocol
- **Source Routing**: Compute and use source routes through the drone network
- **Message Fragmentation**: Handle large messages by fragmenting into 128-byte packets
- **Reliable Communication**: Acknowledgment and error handling with Ack/Nack protocol
- **Simulation Integration**: Compatible with simulation controller for testing

## 📋 Project Structure

```
client-main/
├── src/
│   ├── lib.rs              # Library exports
│   ├── main.rs             # CLI application entry point
│   ├── client.rs           # Main Client struct and core logic
│   ├── assembler.rs        # Message fragmentation and reassembly
│   ├── web_browser.rs      # Web Browser client implementation
│   ├── chat_client.rs      # Chat client implementation
│   └── errors.rs           # Error type definitions
├── examples/
│   └── integration_example.rs  # Comprehensive usage example
└── Cargo.toml             # Dependencies and project metadata
```

## 🛠️ Protocol Implementation

### Network Discovery Protocol ✅
- Implements flood request/response according to AP-protocol.md
- Builds network topology map for route computation
- Handles path traces and node type discovery

### Source Routing Protocol ✅
- BFS-based route computation from client to servers
- Proper SourceRoutingHeader creation with hop indices
- Hop-by-hop packet forwarding through drones

### Fragmentation Protocol ✅
- Messages fragmented into 128-byte data chunks
- Fragment reassembly with sequence tracking
- Proper Ack/Nack handling for reliable delivery

### Client-Server Protocols ✅
- **Web Browser**: `server_type?`, `files_list?`, `file?`, `media?`
- **Chat Client**: `registration_to_chat`, `client_list?`, `message_for?`
- Proper serde serialization matching protocol specification

## 💻 Usage

### Command Line Interface

```bash
# Start a Web Browser client
cargo run -- --id 10 --name "MyWebBrowser" --type web

# Start a Chat client
cargo run -- --id 11 --name "ChatUser" --type chat
```

### Programmatic Usage

```rust
use client::{Client, ClientType};
use crossbeam::channel;

// Create client instance
let client = Client::new(
    10,                                    // Client ID
    "MyWebBrowser".to_string(),            // Client name
    ClientType::WebBrowser,                // Client type
);

// Set up packet communication (normally done by network initializer)
let (packet_sender, packet_receiver) = channel::unbounded();

// Add neighbors (drone connections)
client.add_neighbor(drone_id, drone_sender).await;

// Start the client
client.start(packet_receiver).await?;
```

## 🌐 Web Browser Client Features

- **Server Discovery**: Automatically discover Text and Media servers
- **File Browsing**: Get file lists from Text servers
- **File Retrieval**: Download specific files with size information
- **Media Access**: Retrieve media content from Media servers
- **Interactive Mode**: User-friendly interface for browsing

Example Web Browser operations:
```rust
let web_client = WebBrowserClient::new(id, assembler, network_view);

// Discover servers
let servers = web_client.discover_servers().await?;

// Get file list from text server
let files = web_client.get_files_list(server_id).await?;

// Download a specific file
let (size, data) = web_client.get_file(server_id, "welcome.txt").await?;
```

## 💬 Chat Client Features

- **Server Registration**: Register with Communication servers
- **Client Discovery**: Get list of connected clients
- **Message Sending**: Send messages to other registered clients
- **Message Reception**: Handle incoming messages from other clients
- **Interactive Chat**: Real-time chat session management

Example Chat operations:
```rust
let chat_client = ChatClient::new(id, name, assembler, network_view);

// Find and register to communication servers
let servers = chat_client.discover_communication_servers().await?;
chat_client.register_to_chat(server_id).await?;

// Get connected clients
let clients = chat_client.get_client_list().await?;

// Send a message
chat_client.send_message("target_user", "Hello!").await?;
```

## 🔧 Integration with Network Components

### Network Initializer Integration
The client is designed to work with your network initializer:

1. Network initializer parses TOML configuration
2. Creates client instance with appropriate ID and type
3. Sets up crossbeam channels to connected drones
4. Spawns client thread with packet receiver
5. Client automatically discovers network and starts operation

### Drone Network Integration
- Connects to 1-2 drones as specified in protocol
- Uses source routing through drone network
- Handles packet drops and drone failures
- Implements proper fragmentation for drone packet size limits

### Simulation Controller Integration
- Supports controller commands (AddSender, RemoveSender, Shutdown)
- Reports events (PacketSent, ClientRegistered, MessageSent)
- Compatible with dynamic network topology changes

## 🧪 Testing

Run the integration example to see the client in action:
```bash
cargo run --example integration_example
```

Run unit tests:
```bash
cargo test
```

Check code with clippy:
```bash
cargo clippy
```

## 📚 Protocol Compliance

This implementation fully complies with the AP-protocol.md specification:

- ✅ Network Discovery Protocol with flood request/response
- ✅ Source Routing with proper header handling
- ✅ Message fragmentation into 128-byte packets
- ✅ Fragment reassembly with sequence tracking
- ✅ Ack/Nack protocol for reliable delivery
- ✅ Client-server high-level message protocols
- ✅ Proper NodeId typing and channel management
- ✅ Crossbeam channels for concurrent communication
- ✅ Integration with simulation controller

## 🚧 Future Enhancements

- Add retry logic for failed packet delivery
- Implement connection health monitoring
- Add metrics and logging for network performance
- Support for dynamic server type detection
- Enhanced error recovery mechanisms

## 🤝 Contributing

This client is part of the Advanced Programming 2024/2025 course project. It integrates with:
- Group drone implementations (purchased from other teams)
- Network initializer for topology setup
- Simulation controller for testing and monitoring

The implementation prioritizes protocol compliance, reliability, and clean architecture for easy integration with other project components.