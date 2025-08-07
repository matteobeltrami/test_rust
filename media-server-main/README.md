# Media Server

A media server implementation for the drone network communication system.

## Overview

The Media Server serves multimedia content that is referenced by text files from text servers. It handles various media types including images, videos, audio files, and documents, providing content to web browser clients through the drone network.

## Features

- **Multi-format Media Serving**: Supports images (PNG, JPG, SVG), videos (MP4, WebM), audio (MP3, WAV), and documents (PDF, TXT, ZIP, Rust code)
- **Large File Handling**: Efficiently fragments and transmits large media files
- **Network Discovery**: Automatically discovers network topology using flood protocol
- **Fragment Reassembly**: Handles fragmentation and reassembly for large media content
- **Content Management**: Provides media statistics and content organization

## Media Types Supported

### Images
- PNG: Portable Network Graphics
- JPG: JPEG images  
- SVG: Scalable Vector Graphics

### Videos
- MP4: MPEG-4 video format
- WebM: Open web video format

### Audio
- MP3: MPEG audio compression
- WAV: Uncompressed audio

### Documents  
- PDF: Portable Document Format
- TXT: Plain text files
- ZIP: Compressed archives
- Rust Code: Source code files

## Protocol Support

### Supported Messages

- `server_type?` - Returns server type identification (MediaServer)
- `media?(media_id)` - Returns specific media content by ID
- Error responses for unsupported requests or missing media

### Large File Handling

The media server efficiently handles large files through:
1. Automatic fragmentation into 128-byte chunks
2. Source-routed delivery through drone network
3. Fragment reassembly at destination
4. ACK/NACK protocol for reliable delivery

## Sample Content

The server initializes with various sample media files:
- Network topology diagrams (PNG/SVG)
- System demonstration videos (MP4)
- Performance charts and statistics (PNG)
- Code examples and documentation (Rust/ZIP)
- Tutorial and showcase videos (MP4)

## Usage

```rust
use media_server::MediaServer;

// Create new media server
let mut server = MediaServer::new(server_id);

// Set communication channels (done by network initializer)
server.set_channels(packet_receiver, controller_sender, controller_receiver);

// Add drone neighbors
server.add_neighbor(drone_id, drone_sender).await;

// Start the server
server.run().await?;
```

## Content Management

```rust
// Get media statistics
let stats = server.get_stats().await;
println!("Total files: {}, Total size: {} bytes", stats.total_files, stats.total_size);

// Add new media file
let media_file = MediaFile {
    id: "new_image.png".to_string(),
    name: "New Image".to_string(),
    media_type: MediaType::Image(ImageFormat::Png),
    size: content.len(),
    content: content,
    description: "A new image file".to_string(),
};
server.add_media(media_file).await;
```

## Performance Characteristics

- **Fragment Size**: 128 bytes per fragment for optimal network transmission
- **Concurrent Handling**: Async processing of multiple client requests
- **Memory Management**: Efficient storage and retrieval of large media files
- **Network Optimization**: Source routing minimizes network hops

## Dependencies

- `wg_internal` - Core protocol types and network functionality
- `common` - Shared server foundation and utilities
- `crossbeam` - Channel-based communication
- `tokio` - Async runtime
- `serde` - Message serialization
- `bincode` - Binary message encoding