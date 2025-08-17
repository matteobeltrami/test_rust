# chat-server
# Server (Unified Content and Communication Server)

This repository contains the unified implementation for both content servers (text/media) and communication servers (chat) in the drone network communication system.

## Content Server Section

Unified content server implementation for the drone network communication system. This server consolidates the functionality of both text and media servers into a single, configurable service.

### Features

- **Unified Architecture**: Single server handling both text and media content
- **Multiple Operating Modes**:
    - Text-only mode (serves only text files)
    - Media-only mode (serves only media files)
    - Combined mode (serves both text and media files)
- **Backward Compatibility**: Compatible with existing client implementations
- **Source Routing**: Uses drone network with source routing protocol
- **Fragment Assembly**: Handles large content through fragmentation and reassembly
- **Network Discovery**: Automatic topology discovery and adaptation

### Architecture

The content server consists of several key components:

- **ContentServer**: Main server orchestrator
- **TextHandler**: Manages text file operations and sample content
- **MediaHandler**: Manages media file operations with multiple format support
- **Server**: Base communication layer using crossbeam channels
- **Common Types**: Shared message formats and protocol definitions
