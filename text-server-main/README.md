# Text Server

A text server implementation for the drone network communication system.

## Overview

The Text Server is a content server that provides text files with references to media content. It serves as one of the core components in the drone network architecture, handling requests from web browser clients.

## Features

- **File Serving**: Provides access to text files with structured content
- **Media References**: Text files can reference media content served by media servers
- **Network Discovery**: Automatically discovers network topology using flood protocol
- **Fragment Handling**: Supports fragmentation and reassembly for large messages
- **Reliable Communication**: Uses ACK/NACK protocol for message delivery

## Protocol Support

### Supported Messages

- `server_type?` - Returns server type identification
- `files_list?` - Returns list of available file IDs  
- `file?(file_id)` - Returns specific file content
- Error responses for unsupported requests or missing files

### Message Flow

1. Client sends request message
2. Message is fragmented if > 128 bytes
3. Fragments routed through drone network
4. Server reassembles fragments
5. Server processes request and generates response
6. Response is fragmented and sent back to client

## Sample Content

The server initializes with sample text files including:
- Welcome message with network overview
- Technical specifications
- Getting started guide  
- Network news and updates

## Usage

```rust
use text_server::TextServer;

// Create new text server
let mut server = TextServer::new(server_id);

// Set communication channels (done by network initializer)
server.set_channels(packet_receiver, controller_sender, controller_receiver);

// Add drone neighbors
server.add_neighbor(drone_id, drone_sender).await;

// Start the server
server.run().await?;
```

## Dependencies

- `wg_internal` - Core protocol types and network functionality
- `common` - Shared server foundation and utilities
- `crossbeam` - Channel-based communication
- `tokio` - Async runtime
- `serde` - Message serialization
- `bincode` - Binary message encoding