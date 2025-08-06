# Drone Network Client

A comprehensive Rust client implementation for the Advanced Programming 2024/2025 drone network project. This client supports both Web Browser and Chat Client functionalities as specified in the project requirements.

## Features

### 🌐 Web Browser Client
- **Server Discovery**: Automatically discovers text and media servers in the network
- **File Browsing**: Retrieves and displays file lists from text servers
- **Content Retrieval**: Downloads and displays text files and media content
- **Media Support**: Handles binary media content from media servers
- **Error Handling**: Robust error handling for network and protocol issues

### 💬 Chat Client
- **Server Registration**: Registers with communication servers for messaging
- **Client Discovery**: Finds other clients available for chatting
- **Message Exchange**: Sends and receives messages through communication servers
- **Real-time Communication**: Handles incoming messages asynchronously
- **Session Management**: Manages chat sessions and message history

### 🔧 Core Infrastructure
- **Protocol Compliance**: Full implementation of the drone network protocol
- **Fragment Assembly**: Handles message fragmentation and reassembly
- **Source Routing**: Implements source routing for packet delivery
- **Network Discovery**: Automatic network topology discovery using flood protocol
- **Error Recovery**: Comprehensive error handling and recovery mechanisms
- **Simulation Integration**: Compatible with simulation controller commands

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Web Browser   │    │   Chat Client    │    │  Simulation     │
│     Client      │    │                  │    │  Controller     │
└─────────┬───────┘    └─────────┬────────┘    └─────────┬───────┘
          │                      │                       │
          └──────────┬───────────────────┬───────────────┘
                     │                   │
          ┌──────────▼───────────────────▼───────────┐
          │            Client Core                   │
          │  ┌─────────────┐    ┌─────────────┐      │
          │  │  Assembler  │    │   Network   │      │
          │  │  (Fragment  │    │  Discovery  │      │
          │  │   Handler)  │    │             │      │
          │  └─────────────┘    └─────────────┘      │
          └─────────────┬─────────────────────────────┘
                        │
          ┌─────────────▼─────────────────────────────┐
          │            Drone Network                  │
          │  ┌─────────┐ ┌─────────┐ ┌─────────┐     │
          │  │ Drone 1 │ │ Drone 2 │ │ Drone N │     │
          │  └─────────┘ └─────────┘ └─────────┘     │
          └───────────────────────────────────────────┘
```

## Protocol Implementation

### Network Discovery Protocol
- Implements flood-based network discovery
- Builds topology understanding for source routing
- Handles path trace processing and network updates

### Message Fragmentation
- Automatic message fragmentation for large payloads
- Reliable fragment reassembly with sequence tracking  
- Acknowledgment handling for reliable delivery

### Source Routing
- Computes optimal paths through drone network
- Handles routing failures and path updates
- Supports dynamic network topology changes

## Usage

### Command Line Interface

```bash
# Run Web Browser Client
cargo run -- --id 1 --name "MyBrowser" --type web

# Run Chat Client  
cargo run -- --id 2 --name "MyChat" --type chat
```

### Parameters
- `--id <CLIENT_ID>`: Client ID (0-255)
- `--name <CLIENT_NAME>`: Human-readable client name
- `--type <CLIENT_TYPE>`: Client type (`web` or `chat`)

### Integration with Network Initializer

The client is designed to work with the network initializer. Example configuration:

```toml
# In your network configuration file
[[client]]
id = 4
connected_drone_ids = [1, 2]

[[client]]  
id = 5
connected_drone_ids = [3]
```

## Implementation Details

### Web Browser Protocol Messages

```
Client -> Server: server_type?
Server -> Client: server_type!(type)

Client -> Server: files_list?
Server -> Client: files_list!(list_of_file_ids)

Client -> Server: file?(file_id)
Server -> Client: file!(file_size, file)

Client -> Server: media?(media_id)
Server -> Client: media!(media)
```

### Chat Protocol Messages

```
Client -> Server: registration_to_chat
Client -> Server: client_list?
Server -> Client: client_list!(list_of_client_ids)

Client -> Server: message_for?(client_id, message)
Server -> Client: message_from!(client_id, message)
```

### Error Handling

The client implements comprehensive error handling:

- **Network Errors**: Connection failures, routing issues
- **Protocol Errors**: Invalid responses, malformed packets
- **Fragmentation Errors**: Assembly failures, missing fragments
- **Timeout Errors**: Unresponsive servers or network partitions

## Testing

```bash
# Run tests
cargo test

# Run with verbose output
cargo test -- --nocapture

# Run specific test
cargo test test_client_creation
```

## Dependencies

- **crossbeam**: Concurrent programming and channels
- **tokio**: Async runtime and utilities  
- **serde**: Serialization framework
- **bincode**: Binary serialization format
- **clap**: Command line argument parsing
- **wg_internal**: Drone network protocol library
- **common**: Shared types and utilities

## Project Structure

```
src/
├── main.rs           # CLI entry point
├── lib.rs           # Library exports
├── client.rs        # Core client implementation
├── assembler.rs     # Message fragmentation/assembly
├── web_browser.rs   # Web browser client
├── chat_client.rs   # Chat client implementation
└── errors.rs        # Error types
```

## Development Guidelines

### Code Quality
- No unsafe code
- No undocumented panics
- Comprehensive error handling
- Clean public interfaces only

### Testing Strategy
- Unit tests for core functionality
- Integration tests with mock network
- Protocol compliance verification
- Error condition testing

### Performance Considerations
- Efficient fragment assembly
- Minimal memory copying
- Concurrent message processing
- Timeout-based resource cleanup

## Future Enhancements

- [ ] Persistent message storage
- [ ] Advanced routing algorithms
- [ ] Message encryption support
- [ ] Bandwidth optimization
- [ ] GUI interface
- [ ] Plugin architecture
- [ ] Advanced error recovery
- [ ] Performance metrics

## Contributing

When contributing to this client implementation:

1. Follow Rust best practices and idioms
2. Maintain protocol compliance  
3. Add comprehensive tests
4. Update documentation
5. Ensure backward compatibility

## License

This project is part of the Advanced Programming 2024/2025 course project.