# Content Server

Unified content server implementation for the drone network communication system. This server consolidates the functionality of both text and media servers into a single, configurable service.

## Features

- **Unified Architecture**: Single server handling both text and media content
- **Multiple Operating Modes**: 
  - Text-only mode (serves only text files)
  - Media-only mode (serves only media files)  
  - Combined mode (serves both text and media files)
- **Backward Compatibility**: Compatible with existing client implementations
- **Source Routing**: Uses drone network with source routing protocol
- **Fragment Assembly**: Handles large content through fragmentation and reassembly
- **Network Discovery**: Automatic topology discovery and adaptation

## Architecture

The content server consists of several key components:

- **ContentServer**: Main server orchestrator
- **TextHandler**: Manages text file operations and sample content
- **MediaHandler**: Manages media file operations with multiple format support
- **Server**: Base communication layer using crossbeam channels
- **Common Types**: Shared message formats and protocol definitions

## Usage

### Command Line Interface

```bash
# Start in combined mode (default)
cargo run -- --id 200

# Start in text-only mode
cargo run -- --id 200 --mode text

# Start in media-only mode  
cargo run -- --id 200 --mode media

# Start in combined mode (explicit)
cargo run -- --id 200 --mode combined
```

### Parameters

- `--id, -i`: Server ID (0-255) - Required
- `--mode, -m`: Operating mode - Optional, default: "combined"
  - `text`: Text-only server
  - `media`: Media-only server
  - `combined`: Unified text and media server

## Server Modes

### Text-Only Mode
- Serves text files with media references
- Responds to `FilesListRequest` and `FileRequest` messages
- Reports server type as `TextServer`
- Sample content includes welcome files, technical specs, and guides

### Media-Only Mode  
- Serves multimedia content (images, videos, audio, documents)
- Responds to `MediaRequest` messages
- Reports server type as `MediaServer`
- Supports multiple formats: PNG, JPG, SVG, MP4, WebM, MP3, WAV, PDF, TXT, ZIP, Rust code

### Combined Mode
- Handles both text and media requests
- Can serve complete content experiences (text with referenced media)
- Reports as `TextServer` for compatibility but handles media requests
- Optimal for single-server deployments

## Content Types

### Text Files
```rust
pub struct TextFile {
    pub id: String,
    pub title: String,
    pub content: String,
    pub media_references: Vec<String>, // Referenced media IDs
}
```

### Media Files
```rust
pub struct MediaFile {
    pub id: String,
    pub name: String,
    pub media_type: MediaType,
    pub size: usize,
    pub content: Vec<u8>,
    pub description: String,
}
```

## Protocol Compatibility

The content server maintains full compatibility with the existing client-server protocol:

- **Web Browser Messages**: `ServerTypeRequest`, `FilesListRequest`, `FileRequest`, `MediaRequest`
- **Server Responses**: `ServerType`, `FilesList`, `File`, `Media`, error responses
- **Fragment Assembly**: Automatic handling of large content fragmentation
- **ACK/NACK Protocol**: Reliable delivery with acknowledgments

## Network Integration

- **Drone Network**: Connects through drone neighbors for routing
- **Source Routing**: Computes paths through network topology
- **Network Discovery**: Flood-based topology learning
- **Fault Tolerance**: Handles drone crashes and packet drops
- **Controller Integration**: Receives simulation commands and sends events

## Sample Content

### Text Files
- **welcome**: Introduction to the drone network system
- **technical_specs**: Technical specifications and implementation details
- **getting_started**: Setup and usage guide
- **news**: Latest network updates and statistics

### Media Files
- **network_diagram.png**: Network topology visualization
- **demo_video.mp4**: System demonstration
- **architecture_schema.svg**: Technical architecture diagram
- **performance_chart.png**: Performance metrics
- **code_example.rs**: Implementation examples
- **setup_tutorial.mp4**: Setup walkthrough
- **config_examples.zip**: Configuration templates
- **network_stats.png**: Live statistics dashboard
- **team_showcase.mp4**: Team implementations showcase

## Development

### Dependencies
- Rust 2021 edition
- Tokio async runtime
- Crossbeam channels
- Serde serialization
- Bincode binary encoding
- WGL internal library

### Building
```bash
cargo build --release
```

### Testing
```bash
cargo test
```

### Integration
The content server integrates with:
- Network initializer for topology setup
- Drone implementations for packet routing
- Client applications (web browsers)
- Simulation controller for monitoring

## Migration from Separate Servers

This content server replaces the previous separate `text-server` and `media-server` implementations:

1. **Deployment Simplification**: One server binary instead of two
2. **Resource Efficiency**: Shared infrastructure and connections
3. **Enhanced Functionality**: Combined mode enables richer content experiences
4. **Backward Compatibility**: Existing clients work without modification
5. **Flexible Configuration**: Can operate in legacy modes when needed

The consolidated architecture reduces network complexity while maintaining full feature compatibility with existing implementations.