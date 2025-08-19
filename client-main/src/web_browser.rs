use std::{
    any::Any,
    collections::{hash_map::Entry::Vacant, HashMap, HashSet},
};

use common::{
    types::{File, MediaFile, ServerType, TextFile, WebCommand, WebEvent, WebRequest, WebResponse},
    FragmentAssembler, Processor, RoutingHandler,
};
use crossbeam_channel::{Receiver, Sender};
use uuid::Uuid;
use wg_internal::{
    network::NodeId,
    packet::{NodeType, Packet},
};

type Cache = HashMap<TextFile, Vec<MediaFile>>;

#[derive(Debug)]
pub struct WebBrowser {
    id: NodeId,
    routing_handler: RoutingHandler,
    controller_recv: Receiver<Box<dyn Any>>,
    controller_send: Sender<Box<dyn Any>>,
    packet_recv: Receiver<Packet>,
    assembler: FragmentAssembler,
    text_servers: HashMap<NodeId, Vec<String>>,
    cached_files: Cache,
}

impl WebBrowser {
    #[must_use]
    pub fn new(
        id: NodeId,
        neighbors: HashMap<NodeId, Sender<Packet>>,
        packet_recv: Receiver<Packet>,
        controller_recv: Receiver<Box<dyn Any>>,
        controller_send: Sender<Box<dyn Any>>,
    ) -> Self {
        let routing_handler =
            RoutingHandler::new(id, NodeType::Client, neighbors, controller_send.clone());

        Self {
            id,
            routing_handler,
            controller_recv,
            controller_send,
            packet_recv,
            assembler: FragmentAssembler::default(),
            text_servers: HashMap::new(),
            cached_files: HashMap::new(),
        }
    }

    fn get_text_servers(&self) -> Vec<NodeId> {
        self.text_servers.keys().copied().collect()
    }

    fn get_list_files_by_id(&self, id: NodeId) -> Option<&Vec<String>> {
        self.text_servers.get(&id)
    }

    fn set_files_list(&mut self, server_id: NodeId, list: Vec<String>) {
        if let Vacant(e) = self.text_servers.entry(server_id) {
            e.insert(list);
        }
    }

    fn get_text_files(&self) -> Vec<TextFile> {
        self.cached_files.keys().cloned().collect()
    }

    fn get_text_file(&self, id: Uuid) -> Option<TextFile> {
        self.cached_files.keys().find(|f| f.id == id).cloned()
    }

    // find text_file that contains media file with id
    fn get_text_file_by_media_id(&self, media_id: Uuid) -> Option<TextFile> {
        self.cached_files
            .keys()
            .find(|f| f.get_media_ids().contains(&media_id))
            .cloned()
    }

    fn add_media_file(&mut self, media: MediaFile) {

        let file = self.get_text_file_by_media_id(media.id);
        if let Some(file) = file {
            if let Some(vec) = self.cached_files.get_mut(&file) {
                vec.push(media);
            }
        }
    }

    fn get_files(&self) -> Vec<File> {
        let mut vec = vec![];
        for (text_file, media_files) in &self.cached_files {
            vec.push(File::new(text_file.clone(), media_files.clone()));
        }
        vec
    }

    fn get_file(&self, id: Uuid) -> Option<File> {
        if let Some((tf, m)) = self.cached_files.iter().find(|(f, _)| f.id == id) {
            return Some(File::new(tf.clone(), m.clone()));
        }
        None
    }
}

impl Processor for WebBrowser {
    fn controller_recv(&self) -> &Receiver<Box<dyn Any>> {
        &self.controller_recv
    }

    fn packet_recv(&self) -> &Receiver<Packet> {
        &self.packet_recv
    }

    fn assembler(&mut self) -> &mut FragmentAssembler {
        &mut self.assembler
    }

    fn routing_handler(&mut self) -> &mut RoutingHandler {
        &mut self.routing_handler
    }

    fn handle_command(&mut self, cmd: Box<dyn Any>) -> bool {
        if let Some(cmd) = cmd.downcast_ref::<WebCommand>() {
            match cmd {
                WebCommand::GetCachedFiles => {
                    let files = self.get_files();
                    if self
                        .controller_send
                        .send(Box::new(WebEvent::CachedFiles(files)))
                        .is_err()
                    {
                        return true;
                    }
                }
                WebCommand::GetFile(uuid) => {
                    if let Some(file) = self.get_file(*uuid) {
                        if self
                            .controller_send
                            .send(Box::new(WebEvent::File(file)))
                            .is_err()
                        {
                            return true;
                        }
                    }
                }
                WebCommand::GetTextFiles => {
                    let files = self.cached_files.keys().cloned().collect::<Vec<_>>();
                    if self
                        .controller_send
                        .send(Box::new(WebEvent::TextFiles(files)))
                        .is_err()
                    {
                        return true;
                    }
                }
                WebCommand::GetTextFile(uuid) => {
                    if let Some(file) = self.cached_files.keys().find(|f| f.id == *uuid) {
                        if self
                            .controller_send
                            .send(Box::new(WebEvent::TextFile(file.clone())))
                            .is_err()
                        {
                            return true;
                        }
                    }
                }

                WebCommand::GetMediaFiles => {
                    let mut media = HashSet::new();
                    for v in self.cached_files.values() {
                        for m in v {
                            media.insert(m.clone());
                        }
                    }

                    let media = media.into_iter().collect();
                    if self
                        .controller_send
                        .send(Box::new(WebEvent::MediaFiles(media)))
                        .is_err()
                    {
                        return true;
                    }
                }
                WebCommand::GetMediaFile(uuid) => {
                    for v in self.cached_files.values() {
                        if let Some(media) = v.iter().find(|m| m.id == *uuid) {
                            if self
                                .controller_send
                                .send(Box::new(WebEvent::MediaFile(media.clone())))
                                .is_err()
                            {
                                return true;
                            }
                            break;
                        }
                    }
                }
                _ => {
                    eprintln!("Unsupported command: {:?}", cmd);
                    todo!()
                }
            }
        }

        false
    }

    fn handle_msg(&mut self, msg: Vec<u8>, from: NodeId, session_id: u64) {
        if let Ok(msg) = serde_json::from_slice::<WebResponse>(&msg) {
            match msg {
                WebResponse::ServerType { server_type } => {
                    if matches!(server_type, ServerType::TextServer) {
                        self.text_servers.insert(from, vec![]);
                    }
                }
                WebResponse::TextFilesList { files } => {
                    self.set_files_list(from, files);
                }
                WebResponse::TextFile { file_data } => {
                    if let Ok(file) = serde_json::from_slice::<TextFile>(&file_data) {
                        for r in &file.get_refs() {
                            if let Ok(req) = serde_json::to_vec(&WebRequest::MediaQuery {
                                media_id: r.id.to_string(),
                            }) {
                                let _ = self.routing_handler.send_message(
                                    &req,
                                    r.get_location(),
                                    Some(session_id),
                                );
                            }
                        }
                        let _ = self.cached_files.insert(file, vec![]);
                    }
                }
                WebResponse::MediaFile { media_data } => {
                    if let Ok(file) = serde_json::from_slice::<MediaFile>(&media_data) {
                        self.add_media_file(file);
                    }
                }

                WebResponse::ErrorFileNotFound(uuid) => {
                    let _ = self
                        .controller_send
                        .send(Box::new(WebEvent::FileNotFound(uuid)));
                }
            }
        }
    }
}
