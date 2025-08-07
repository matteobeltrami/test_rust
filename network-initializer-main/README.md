# Network Initializer

The network initializer is responsible for setting up and starting the entire drone network system, including drones, servers, and the infrastructure for clients to connect.

## Overview

The Network Initializer reads a TOML configuration file that defines the network topology and spawns all components according to the specification. It manages the distribution of drone implementations from different teams and ensures proper connectivity between all network nodes.

## Features

- **Configuration Parsing**: Reads and validates TOML network configuration files
- **Drone Distribution**: Evenly distributes different drone implementations across the network
- **Server Management**: Spawns text servers, media servers, and communication servers
- **Channel Setup**: Creates and manages all crossbeam channels for inter-node communication
- **Network Validation**: Ensures configuration meets topology requirements before startup

## Drone Implementations

The network initializer uses drone implementations from multiple teams:

1. **null-pointer-drone**: Robust error handling implementation
2. **wg_2024-rust**: High performance drone implementation
3. **d-r-o-n-e_drone**: Advanced routing algorithms
4. **rusteze_drone**: Optimized for low latency
5. **ap2024_unitn_cppenjoyers_drone**: Feature-rich implementation
6. **dr_ones**: Innovative approach to packet handling
7. **rusty_drones**: Reliability-focused implementation
8. **rustbusters-drone**: Performance-optimized design
9. **rustafarian-drone**: Creative networking solutions
10. **lockheedrustin-drone**: Industrial-grade implementation

## Server Types

### Text Servers
- Serve text files with media references
- Handle file listing and retrieval requests
- Connect to at least 2 drones for reliability

### Media Servers  
- Provide multimedia content (images, videos, documents)
- Handle large file fragmentation and transmission
- Support various media formats (PNG, JPG, MP4, etc.)

### Communication Servers
- Enable real-time messaging between chat clients
- Manage client registration and discovery
- Forward messages between registered users

## Configuration Format

The network uses TOML configuration files with the following structure:

```toml
# Drone configuration
[[drone]]
id = 1
connected_node_ids = [2, 3, 4]
pdr = 0.1  # Packet drop rate (0.0 - 1.0)

[[drone]]
id = 2
connected_node_ids = [1, 5]
pdr = 0.05

# Client configuration
[[client]]
id = 100
connected_drone_ids = [1, 2]  # Max 2 connections

# Server configuration  
[[server]]
id = 200
connected_drone_ids = [2, 3]  # Min 2 connections
```

## Configuration Validation

The initializer validates configurations to ensure:

- **Connected Graph**: All nodes are reachable through the network
- **Edge Placement**: Clients and servers are at network edges
- **Connection Limits**: Clients ≤ 2 drones, servers ≥ 2 drones
- **Unique IDs**: No duplicate node identifiers
- **Bidirectional Links**: All connections are properly bidirectional
- **Valid PDR**: Packet drop rates between 0.0 and 1.0

## Usage

### Command Line Interface

```bash
# Basic usage with default server counts
./network-initializer -c config.toml

# Custom server configuration
./network-initializer -c config.toml \
    --text-servers 2 \
    --media-servers 1 \
    --chat-servers 3
```

### Command Line Options

- `-c, --config <CONFIG_FILE>`: Path to network configuration file (required)
- `--text-servers <COUNT>`: Number of text servers to spawn (default: 1)
- `--media-servers <COUNT>`: Number of media servers to spawn (default: 1)  
- `--chat-servers <COUNT>`: Number of communication servers to spawn (default: 1)

### Example Startup

```bash
🚀 Starting Network Initializer
   Config file: examples/network.toml
   Text servers: 1
   Media servers: 1
   Chat servers: 1

✅ Configuration validated successfully
   Drones: 10
   Clients: 0
   Servers: 0

📡 Setting up communication channels...
🚁 Starting Drone 1 with implementation: null-pointer
🚁 Starting Drone 2 with implementation: wg2024-rust
...
🗃️  Starting Text Server 1
🖼️  Starting Media Server 1
💬 Starting Communication Server 1

✅ All nodes started successfully
✅ Network initialization completed
🔄 Network is now running - press Ctrl+C to stop
```

## Architecture

### Initialization Process

1. **Parse Configuration**: Load and validate TOML configuration file
2. **Create Channels**: Set up unbounded crossbeam channels for all nodes
3. **Distribute Drones**: Assign drone implementations using round-robin distribution
4. **Spawn Drones**: Start drone threads with proper channel connections
5. **Start Servers**: Launch server instances with drone connectivity
6. **Monitor Network**: Keep system running until shutdown

### Channel Management

The initializer creates several types of channels:

- **Packet Channels**: For routing data packets between nodes
- **Controller Channels**: For simulation controller commands to drones
- **Event Channels**: For drones to report events to controller

### Thread Architecture

Each network component runs in its own thread:
- **Drone Threads**: Handle packet routing and network protocols
- **Server Threads**: Process client requests and serve content
- **Main Thread**: Monitors system and handles shutdown

## Network Topology Requirements

### Connectivity
- Must form a connected graph where all nodes can reach each other
- Removing clients and servers must still leave a connected drone network
- No isolated nodes or network partitions allowed

### Node Placement
- **Clients**: Maximum 2 drone connections (edge nodes)
- **Servers**: Minimum 2 drone connections (edge nodes with redundancy)
- **Drones**: Form the core routing infrastructure

### Reliability Features
- **Multiple Paths**: Servers connect to multiple drones for fault tolerance
- **Implementation Diversity**: Different drone implementations provide robustness
- **Error Handling**: Comprehensive error reporting and recovery

## Dependencies

- `wg_internal`: Core protocol definitions and network types
- Multiple drone implementations from different teams
- Server implementations (text-server, media-server, chat-server)  
- `crossbeam`: Channel-based inter-thread communication
- `tokio`: Async runtime for server operations
- `clap`: Command line argument parsing
- `toml`: Configuration file parsing

## Development

### Adding New Drone Implementations

1. Add dependency to `Cargo.toml`
2. Import in `main.rs`  
3. Add to drone selection logic
4. Update implementation array

### Extending Server Types

1. Create new server library
2. Add dependency to network initializer
3. Implement server startup logic
4. Add command line options for configuration

## Troubleshooting

### Common Issues

- **Configuration Errors**: Check TOML syntax and validation rules
- **Channel Errors**: Verify all nodes have proper channel connections
- **Threading Issues**: Monitor thread spawning and error handling
- **Network Connectivity**: Ensure topology meets connectivity requirements

### Debug Information

The initializer provides detailed logging during startup:
- Configuration parsing results
- Channel creation progress  
- Node startup confirmation
- Error messages with context