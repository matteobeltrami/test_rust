use wg_internal::network::{ NodeId, SourceRoutingHeader };
use std::sync::Arc;
use wg_internal::packet::{ Packet, FloodRequest, PacketType, NodeType};
use crossbeam::channel::{ Sender, Receiver };
use std::collections::HashMap;
use common::network::{Network, Node};
use tokio::sync::{oneshot, Mutex, RwLock};


type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<PacketType>>>>;

pub struct Client {
    neighbors: HashMap<NodeId, Sender<Packet>>,
    id: NodeId,
    network_view: Arc<RwLock<Network>>,
    last_discovery: Option<u64>,
    session_id: Arc<RwLock<u64>>,
    listener: tokio::task::JoinHandle<()>,
    pending: Pending,
}


impl Client {
    pub fn new(id: NodeId, receiver: Receiver<Packet>) -> Self {
        let pending =Arc::new(Mutex::new(HashMap::new()));
        Self {
            id,
            neighbors: HashMap::new(),
            network_view: Arc::new(RwLock::new(Network::new(Node::new(id, NodeType::Client, vec![])))),
            session_id: Arc::new(RwLock::new(0)),
            last_discovery: None,
            listener: Listener::new(receiver, pending.clone()).listen(),
            pending
        }
    }

    pub fn discover(&mut self) -> tokio::task::JoinHandle<()> {
        // This function return the future use to handle network discovery

        self.last_discovery = if let Some(flood_id) = self.last_discovery {
            Some(flood_id + 1)
        } else {
            Some(0)
        };

        let mut neighbors = self.neighbors.clone();
        let flood_id = self.last_discovery.unwrap();
        let client_id= self.id;
        let session_id= self.session_id.clone();
        let pending = self.pending.clone();
        let net_view = self.network_view.clone();
        tokio::task::spawn(async move {

            for (id, sender) in neighbors.iter_mut() {
                match sender.send(
                    Packet::new_flood_request(
                        SourceRoutingHeader::empty_route(),
                        *session_id.read().await,
                        FloodRequest::new(flood_id, client_id )
                    )
                ) {
                    Ok(_) => {
                        let (tx, rx) = oneshot::channel();

                        {

                            let mut lock = pending.lock().await;
                            lock.insert(*session_id.read().await, tx);
                        }

                        let _ = session_id.write().await.saturating_add(1);

                        let packet = rx.await.expect("Cannot receive from oneshot channel");
                        match packet {
                            PacketType::FloodResponse(flood_response) => {
                                let neighbors = flood_response.path_trace
                                    .iter()
                                    .enumerate()
                                    .map(|(i, &(node_id, node_type))| {
                                        let mut neigh = Vec::new();

                                        if i > 0 {
                                            neigh.push(flood_response.path_trace[i - 1]);
                                        }

                                        if i + 1 < flood_response.path_trace.len() {
                                            neigh.push(flood_response.path_trace[i + 1]);
                                        }

                                        ((node_id, node_type), neigh)
                                    })
                                .collect::<Vec<_>>();

                                for (node, neighbors) in neighbors.iter() {
                                    let (id, node_type) = node;
                                    let neigh_ids: Vec<u8> = neighbors.iter().map(|(n_id, _)| *n_id).collect();
                                    match net_view.write().await.update_node(*id, neigh_ids.clone()) {
                                        Ok(_) => {},
                                        Err(_) => {
                                            let _ = net_view.write().await.add_node(Node::new(*id, *node_type, neigh_ids));

                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(_) => eprintln!("Failed to send message to {}", id),
                }
            }
        })
    }

}


struct Listener {
    rx: Option<Receiver<Packet>>,
    pending: Option<Pending>
}


impl Listener {
    fn new (rx: Receiver<Packet>, pending: Pending) -> Self {
        Self { rx: Some(rx), pending: Some(pending) }
    }

    fn listen(&mut self) -> tokio::task::JoinHandle<()>{
        let rx = self.rx.take().expect("Receiver channel already taken.");
        let pending = self.pending.take().expect("Pending request map already taken.");
        tokio::task::spawn( async move {
            loop {
                match rx.try_recv(){
                    Ok(pkt) => {
                        let mut lock = pending.lock().await;
                        if let Some(tx) = lock.remove(&pkt.session_id) {
                            let _ = tx.send(pkt.pack_type);
                        }

                    },
                    Err(_) => {}
                }

            }
        })
    }
}

