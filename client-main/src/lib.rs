pub mod client;
pub mod errors;
pub mod assembler;
pub mod web_browser;
pub mod chat_client;

// Re-export main types for easier use
pub use client::Client;
pub use web_browser::WebBrowserClient;
pub use chat_client::ChatClient;
pub use assembler::Assembler;