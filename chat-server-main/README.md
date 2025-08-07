# Communication Server (Chat Server)

A communication server implementation for the drone network communication system.

## Overview

The Communication Server facilitates real-time messaging between chat clients in the drone network. It handles client registration, maintains client lists, and forwards messages between registered users, acting as a central hub for chat communications.

## Features

- **Client Registration**: Allows clients to register with unique names
- **Client Discovery**: Provides lists of registered clients to other clients
- **Message Forwarding**: Routes messages between registered clients
- **Message History**: Maintains a history of recent chat messages
- **Activity Tracking**: Tracks client activity and registration times
- **Network Discovery**: Automatically discovers network topology using flood protocol
- **Reliable Communication**: Uses fragmentation and ACK/NACK for message delivery

## Protocol Support

### Supported Messages

- `server_type?` - Returns server type identification (CommunicationServer)
- `registration_to_chat(client_name)` - Registers a client with the given name
- `client_list?` - Returns list of all registered client names
- `message_for?(client_name, message)` - Forwards message to specified client
- `message_from!(client_name, message)` - Delivers message from another client
- Error responses for invalid operations or unknown clients

### Message Flow

#### Client Registration
1. Client sends `registration_to_chat("username")`
2. Server validates name availability
3. Server registers client and returns current client list
4. Registration confirmed or error returned

#### Message Exchange
1. Client A sends `message_for?("ClientB", "Hello")`
2. Server validates target client exists
3. Server forwards as `message_from!("ClientA", "Hello")` to Client B
4. Server updates message history and activity tracking

#### Client Discovery
1. Client sends `client_list?`
2. Server returns list of all registered client names
3. Client can use list to select message targets

## Communication Features

### Client Management
- **Unique Names**: Each client must register with a unique name
- **Registration Tracking**: Stores registration time and last activity
- **Activity Monitoring**: Tracks which clients are currently active
- **Name-to-ID Mapping**: Maintains bidirectional client name/ID lookup

### Message Handling
- **Real-time Forwarding**: Messages delivered immediately to online clients
- **Message History**: Stores recent messages for analysis and debugging
- **Error Handling**: Proper error responses for invalid targets or operations
- **Activity Updates**: Client activity updated on each message sent

### Statistics and Monitoring
- **Server Uptime**: Tracks how long the server has been running
- **Client Statistics**: Total registered, currently active clients
- **Message Metrics**: Total messages processed, per-client counts
- **History Management**: Automatic cleanup to prevent memory overflow

## Usage

```rust
use chat_server::CommunicationServer;

// Create new communication server
let mut server = CommunicationServer::new(server_id);

// Set communication channels (done by network initializer)
server.set_channels(packet_receiver, controller_sender, controller_receiver);

// Add drone neighbors
server.add_neighbor(drone_id, drone_sender).await;

// Start the server
server.run().await?;
```

## Administrative Functions

```rust
// Get server statistics
let stats = server.get_stats().await;
println!("Active clients: {}/{}", stats.active_clients, stats.total_clients);
println!("Total messages: {}", stats.total_messages);

// Get registered clients
let clients = server.get_registered_clients().await;
for client in clients {
    println!("Client: {} (ID: {}, Last active: {})", 
             client.name, client.id, client.last_activity);
}

// Get recent message history
let recent_messages = server.get_message_history(Some(10)).await;
for msg in recent_messages {
    println!("{}: {} -> {}: {}", 
             msg.timestamp, msg.from_client_name, msg.to_client_name, msg.message);
}

// Remove inactive client
server.remove_client(inactive_client_id).await;
```

## Error Handling

The server provides specific error responses for various scenarios:

- **Name Already Taken**: `ErrorWrongClientId` when registration name conflicts
- **Invalid Target**: `ErrorWrongClientId` when message target doesn't exist  
- **Unsupported Request**: `ErrorUnsupportedRequest` for invalid message types

## Performance Characteristics

- **Concurrent Clients**: Supports multiple simultaneous client connections
- **Message History**: Maintains last 1000 messages automatically
- **Activity Tracking**: 5-minute timeout for active client determination
- **Memory Management**: Automatic cleanup prevents unbounded growth
- **Network Efficiency**: Uses source routing for optimal message paths

## Integration with Other Components

### With Chat Clients
- Clients register using unique names
- Clients discover other available users
- Real-time bidirectional messaging

### With Network Infrastructure  
- Connects to multiple drones for reliability
- Uses network discovery to map topology
- Handles fragment reassembly for large messages
- Integrates with simulation controller for monitoring

## Dependencies

- `wg_internal` - Core protocol types and network functionality
- `common` - Shared server foundation and utilities
- `crossbeam` - Channel-based communication
- `tokio` - Async runtime
- `serde` - Message serialization
- `bincode` - Binary message encoding