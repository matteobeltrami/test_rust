/// Example demonstrating how the client integrates with the drone network
/// This shows the expected usage pattern when used with the network initializer

use client::Client;
use common::types::ClientType;
use wg_internal::packet::Packet;
use crossbeam::channel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Drone Network Client Integration Example");
    println!("This demonstrates how clients work within the full network setup\n");

    // === SCENARIO 1: Web Browser Client ===
    println!("📋 SCENARIO 1: Web Browser Client");
    println!("Setting up a web browser client that will:");
    println!("  1. Discover the network topology");
    println!("  2. Find text and media servers");  
    println!("  3. Browse and retrieve content");
    
    let web_client = setup_web_browser_client().await?;
    simulate_web_browsing(&web_client).await?;
    
    println!("\n{}\n", "=".repeat(60));
    
    // === SCENARIO 2: Chat Client ===
    println!("📋 SCENARIO 2: Chat Client");  
    println!("Setting up a chat client that will:");
    println!("  1. Register with a communication server");
    println!("  2. Discover other connected clients");
    println!("  3. Exchange messages");
    
    let chat_client = setup_chat_client().await?;
    simulate_chat_session(&chat_client).await?;
    
    println!("\n{}\n", "=".repeat(60));
    
    // === SCENARIO 3: Network Integration ===
    println!("📋 SCENARIO 3: Full Network Integration");
    println!("This shows how clients integrate with:");
    println!("  • Network Initializer");
    println!("  • Drone Network"); 
    println!("  • Simulation Controller");
    
    demonstrate_network_integration().await?;
    
    println!("\n✅ Integration examples completed successfully!");
    
    Ok(())
}

async fn setup_web_browser_client() -> Result<Client, Box<dyn std::error::Error>> {
    println!("🌐 Creating Web Browser Client...");
    
    // Create packet channel (normally handled by network initializer)
    let (_sender, _receiver): (crossbeam::channel::Sender<Packet>, crossbeam::channel::Receiver<Packet>) = channel::unbounded();
    
    // Create client
    let client = Client::new(
        10, // Client ID
        "DemoWebBrowser".to_string(),
        ClientType::WebBrowser,
    );
    
    // In real usage, network initializer would:
    // 1. Create the client
    // 2. Set up neighbor connections to drones
    // 3. Start the client thread
    
    println!("   ✅ Web browser client created (ID: 10)");
    Ok(client)
}

async fn simulate_web_browsing(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Simulating web browsing activities...");
    
    // In a real scenario, this would:
    println!("   📡 Discovery: Find text servers (ID: 20, 21)");
    println!("   📡 Discovery: Find media servers (ID: 22, 23)"); 
    println!("   📄 Request: Get file list from text server 20");
    println!("   📄 Response: ['welcome.txt', 'about.html', 'news.md']");
    println!("   📖 Request: Get file 'welcome.txt' from server 20");
    println!("   📖 Response: 'Welcome to the drone network!' (245 bytes)");
    println!("   🎭 Request: Get media 'logo.png' from server 22");
    println!("   🎭 Response: Binary media data (1024 bytes)");
    
    // Simulate the protocol flow
    println!("   🔄 Protocol: Fragmentation (3 fragments)");
    println!("   🔄 Protocol: Source routing through drones [10→1→3→20]");
    println!("   🔄 Protocol: Acknowledgments received");
    
    println!("   ✅ Web browsing simulation complete");
    Ok(())
}

async fn setup_chat_client() -> Result<Client, Box<dyn std::error::Error>> {
    println!("💬 Creating Chat Client...");
    
    let client = Client::new(
        11, // Client ID  
        "DemoChatUser".to_string(),
        ClientType::ChatClient,
    );
    
    println!("   ✅ Chat client created (ID: 11, Name: 'DemoChatUser')");
    Ok(client)
}

async fn simulate_chat_session(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("💭 Simulating chat session activities...");
    
    // In a real scenario:
    println!("   📡 Discovery: Find communication servers (ID: 30, 31)");
    println!("   📝 Register: Register 'DemoChatUser' with server 30");
    println!("   👥 Query: Get client list from server 30");  
    println!("   👥 Response: ['Alice', 'Bob', 'Charlie', 'DemoChatUser']");
    println!("   💌 Send: Message to 'Alice': 'Hello from the drone network!'");
    println!("   📨 Receive: Message from 'Bob': 'Welcome to the chat!'");
    println!("   💌 Send: Message to 'Charlie': 'How's the weather up there?'");
    println!("   📨 Receive: Message from 'Alice': 'Hi! Great to see you here!'");
    
    // Simulate protocol details
    println!("   🔄 Protocol: Message fragmentation (2 fragments per message)");
    println!("   🔄 Protocol: Bidirectional routing [11↔30↔other_clients]");
    println!("   🔄 Protocol: Real-time message delivery");
    
    println!("   ✅ Chat session simulation complete");
    Ok(())
}

async fn demonstrate_network_integration() -> Result<(), Box<dyn std::error::Error>> {
    println!("🕸️  Demonstrating full network integration...");
    
    // Show network topology
    println!("\n   🗺️  Network Topology:");
    println!("   ┌─────────────────────────────────────────────────────┐");
    println!("   │  Client 10 (Web)    Client 11 (Chat)               │");
    println!("   │      │                   │                         │");
    println!("   │   Drone 1 ──────────── Drone 2 ──────────── Drone 3│");
    println!("   │      │                   │                    │    │");
    println!("   │  Server 20 (Text)   Server 30 (Comm)   Server 22   │");
    println!("   │  Server 21 (Text)   Server 31 (Comm)   (Media)     │");
    println!("   └─────────────────────────────────────────────────────┘");
    
    // Show initialization process
    println!("\n   📋 Network Initializer Process:");
    println!("   1. Parse network configuration (TOML)");
    println!("   2. Validate topology and constraints");
    println!("   3. Create crossbeam channels between nodes");
    println!("   4. Spawn drone threads with purchased implementations");
    println!("   5. Create client instances");
    println!("   6. Set up client-drone connections");
    println!("   7. Start simulation controller");
    println!("   8. Begin packet processing loops");
    
    // Show runtime behavior
    println!("\n   ⚡ Runtime Behavior:");
    println!("   • Clients automatically discover network topology");
    println!("   • Messages are fragmented and routed through drones");
    println!("   • Dropped packets trigger retransmission");
    println!("   • Drone crashes are detected and routes updated");
    println!("   • Simulation controller can modify network dynamically");
    
    // Show protocol compliance
    println!("\n   📋 Protocol Compliance:");
    println!("   ✅ Network Discovery Protocol implemented");
    println!("   ✅ Source Routing with hop-by-hop forwarding");
    println!("   ✅ Message fragmentation and reassembly");
    println!("   ✅ Acknowledgment and error handling");
    println!("   ✅ High-level client-server protocols");
    println!("   ✅ Simulation controller integration");
    
    println!("\n   ✅ Network integration demonstration complete");
    Ok(())
}

/// Example showing how network initializer would set up the client
fn example_network_initializer_integration() {
    println!("📋 Example: Network Initializer Integration");
    println!();
    
    // This is pseudocode showing the expected integration:
    println!("```rust");
    println!("// In network initializer:");
    println!("let (client_sender, client_receiver) = channel::unbounded();");
    println!("let (drone1_sender, drone1_receiver) = channel::unbounded();"); 
    println!("let (drone2_sender, drone2_receiver) = channel::unbounded();");
    println!();
    println!("// Create client");
    println!("let client = Client::new(4, \"WebClient\".to_string(), ClientType::WebBrowser);");
    println!();
    println!("// Set up connections to drones");
    println!("client.add_neighbor(1, drone1_sender).await;");
    println!("client.add_neighbor(2, drone2_sender).await;");
    println!();
    println!("// Start client processing");
    println!("tokio::spawn(async move {{");
    println!("    client.start(client_receiver).await");
    println!("}});");
    println!("```");
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_client_creation() {
        let client = Client::new(1, "TestClient".to_string(), ClientType::WebBrowser);
        assert_eq!(client.get_id(), 1);
        assert_eq!(client.get_name(), "TestClient");
    }
    
    #[tokio::test]
    async fn test_both_client_types() {
        let web_client = Client::new(1, "WebTest".to_string(), ClientType::WebBrowser);
        let chat_client = Client::new(2, "ChatTest".to_string(), ClientType::ChatClient);
        
        assert_eq!(web_client.get_id(), 1);
        assert_eq!(chat_client.get_id(), 2);
    }
}