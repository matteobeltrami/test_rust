pub mod server;
pub mod errors;
pub mod assembler;
pub mod content_server;
pub mod text_handler;
pub mod media_handler;

// Re-export main types for easier use
pub use server::Server;
pub use content_server::{ContentServer, ServerMode};
pub use text_handler::{TextFile, TextHandler};
pub use media_handler::{MediaFile, MediaHandler, MediaType, ImageFormat, VideoFormat, AudioFormat, DocumentFormat, MediaStats};
pub use assembler::Assembler;