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

#[cfg(test)]
mod web_browser_tests {
    use super::*;
    use crossbeam::channel::unbounded;
    use common::types::{WebResponse, ServerType, TextFile, MediaFile, MediaReference};

    fn create_test_web_browser() -> WebBrowser {
        let (controller_send, controller_recv) = unbounded();
        let (_, packet_recv) = unbounded();
        let neighbors = HashMap::new();

        WebBrowser::new(1, neighbors, packet_recv, controller_recv, controller_send)
    }

    #[test]
    /// Tests ServerType response handling (text server being added to HashSet)
    fn test_server_type_identification() {
        let mut browser = create_test_web_browser();

        let response = WebResponse::ServerType {
            server_type: ServerType::TextServer
        };
        let serialized = serde_json::to_vec(&response).unwrap();
        browser.handle_msg(serialized, 5, 100);

        assert!(browser.text_servers.contains_key(&5));
    }

    #[test]
    /// Tests TextFilesList response handling (server files list being added to HashSet)
    fn test_text_files_list_handling() {
        let mut browser = create_test_web_browser();

        let files = vec![
            "file1-id:Article 1".to_string(),
            "file2-id:Article 2".to_string()
        ];
        let response = WebResponse::TextFilesList { files };
        let serialized = serde_json::to_vec(&response).unwrap();
        browser.handle_msg(serialized, 5, 101);

        let server_files = browser.get_list_files_by_id(5).unwrap();
        assert_eq!(server_files.len(), 2);
    }

    #[test]
    /// Tests TextFile handling and caching
    fn test_text_file_caching_with_media_refs() {
        let mut browser = create_test_web_browser();

        let media_ref = MediaReference::new(6);
        let text_file = TextFile::new(
            "Article with Media".to_string(),
            "Content with media ref".to_string(),
            vec![media_ref]
        );
        let serialized_file = serde_json::to_vec(&text_file).unwrap();
        let response = WebResponse::TextFile {
            file_data: serialized_file
        };
        let serialized = serde_json::to_vec(&response).unwrap();
        browser.handle_msg(serialized, 5, 102);

        assert!(browser.cached_files.contains_key(&text_file));
        let cached_files = browser.get_text_files();
        assert_eq!(cached_files.len(), 1);
    }

    #[test]
    /// Tests association between TextFile and Media File in File
    fn test_media_file_association() {
        let mut browser = create_test_web_browser();

        let media_ref = MediaReference::new(6);
        let text_file = TextFile::new(
            "Article".to_string(),
            "Content".to_string(),
            vec![media_ref.clone()]
        );
        browser.cached_files.insert(text_file.clone(), vec![]);
        let media_file = MediaFile {
            id: media_ref.id,
            title: "Test Image".to_string(),
            content: vec![vec![1, 2, 3, 4]]
        };
        let serialized_media = serde_json::to_vec(&media_file).unwrap();
        let response = WebResponse::MediaFile {
            media_data: serialized_media
        };
        let serialized = serde_json::to_vec(&response).unwrap();
        browser.handle_msg(serialized, 6, 103);

        let files = browser.get_files();
        assert_eq!(files.len(), 1);
    }

    #[test]
    /// Tests if different commands do not panick
    fn test_command_responses() {
        let mut browser = create_test_web_browser();

        let text_file = TextFile::new("Test".to_string(), "Content".to_string(), vec![]);
        let file_id = text_file.id;
        browser.cached_files.insert(text_file, vec![]);

        let commands = vec![
            WebCommand::GetCachedFiles,
            WebCommand::GetFile(file_id),
            WebCommand::GetTextFiles,
            WebCommand::GetTextFile(file_id),
            WebCommand::GetMediaFiles,
        ];

        for cmd in commands {
            let should_continue = browser.handle_command(Box::new(cmd));
            assert!(!should_continue);
        }
    }
}