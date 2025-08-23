use std::collections::{hash_map::Entry::Vacant, HashMap, HashSet};
use common::{
    types::{
        Command, Event, File, MediaFile, NodeCommand, ServerType, TextFile, WebCommand, WebEvent, WebRequest, WebResponse
    },
    FragmentAssembler, Processor, RoutingHandler,
};
use crossbeam_channel::{Receiver, Sender};
use uuid::Uuid;
use wg_internal::{
    network::NodeId,
    packet::{NodeType, Packet},
};
use crate::errors::ClientError;

type Cache = HashMap<TextFile, Vec<MediaFile>>;

#[derive(Debug)]
pub struct WebBrowser {
    id: NodeId,
    routing_handler: RoutingHandler,
    controller_recv: Receiver<Box<dyn Command>>,
    controller_send: Sender<Box<dyn Event>>,
    packet_recv: Receiver<Packet>,
    assembler: FragmentAssembler,
    text_servers: HashMap<NodeId, Vec<String>>, // id, file_list
    cached_files: Cache,
    pending_request: Option<WebRequest>,
}

impl WebBrowser {
    #[must_use]
    pub fn new(
        id: NodeId,
        neighbors: HashMap<NodeId, Sender<Packet>>,
        packet_recv: Receiver<Packet>,
        controller_recv: Receiver<Box<dyn Command>>,
        controller_send: Sender<Box<dyn Event>>,
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
            pending_request: None,
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

    fn manage_media_file(&mut self, media: MediaFile) {
        if let Some(file) = self.get_text_file_by_media_id(media.id) {
            if let Some(vec) = self.cached_files.get_mut(&file) {
                vec.push(media);
                if file.get_media_ids().len() == vec.len() {
                    let _ = self
                        .controller_send
                        .send(Box::new(WebEvent::File(File::new(file, vec.clone()))));
                }
            }
        } else {
            let _ = self
                .controller_send
                .send(Box::new(WebEvent::MediaFile(media)));
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

    fn locate_file(&self, uuid: Uuid) -> Option<NodeId> {
        for (server, file_list) in &self.text_servers {
            if file_list.contains(&uuid.to_string()) {
                return Some(*server);
            }
        }
        None
    }

    fn manage_text_file(&mut self, file: TextFile, session_id: u64) {
        for r in &file.get_refs() {
            if let Ok(req) = serde_json::to_vec(&WebRequest::MediaQuery {
                media_id: r.id.to_string(),
            }) {
                let _ = self
                    .routing_handler
                    .send_message(&req, r.get_location(), Some(session_id));
            }
        }
        if file.get_refs().is_empty() {
            let _ = self
                .controller_send
                .send(Box::new(WebEvent::File(File::new(file.clone(), vec![]))));
        }
        let _ = self.cached_files.insert(file, vec![]);
    }

    fn try_send(&self, event: WebEvent) -> bool {
        self.controller_send.send(Box::new(event)).is_err()
    }

    // TODO: Create Custom errors (WebBrowserError) of type (NoLocation, SerializeError,
    // UuidParaseError)
    fn forward_request(&mut self, req: &WebRequest) -> Result<(), ClientError> {
        if let Ok(serialized) = serde_json::to_vec(req) {
            if let Some(uuid) = req.get_file_id() {
                if let Ok(uuid) = Uuid::parse_str(&uuid) {
                    if let Some(location) = self.locate_file(uuid) {
                        let _ = self
                            .routing_handler
                            .send_message(&serialized, location, None);
                        return Ok(());
                    }
                    return Err(ClientError::NoLocationError);
                }
                return Err(ClientError::UuidParseError);
            }
        } else {
            return Err(ClientError::SerializationError);
        }

        Ok(())
    }

    fn handle_get_cached_files(&self) -> bool {
        let files = self.get_files();
        self.try_send(WebEvent::CachedFiles(files))
    }

    fn broadcast(&mut self) {
        if let Some(servers) = self.routing_handler.get_servers() {
            for s in servers {
                if let Ok(req) = serde_json::to_vec(&WebRequest::ServerTypeQuery) {
                    let _ = self.routing_handler.send_message(&req, s, None);
                }
            }
        }
    }

    fn handle_get_file(&mut self, uuid: Uuid) -> bool {
        if let Some(file) = self.get_file(uuid) {
            return self.try_send(WebEvent::File(file));
        }
        match self.forward_request(&WebRequest::FileQuery {
            file_id: uuid.to_string(),
        }) {
            Ok(()) => return false,
            Err(ClientError::NoLocationError) => {
                self.broadcast();
                self.pending_request = Some(WebRequest::FileQuery {
                    file_id: uuid.to_string(),
                });
            }
            Err(e) => {
                eprintln!("Error forwarding request: {e}");
                return false;
            }
        }

        false
    }

    fn handle_get_text_files(&self) -> bool {
        let files = self.cached_files.keys().cloned().collect::<Vec<_>>();
        self.try_send(WebEvent::TextFiles(files))
    }

    fn handle_get_text_file(&mut self, uuid: Uuid) -> bool {
        if let Some(file) = self.cached_files.keys().find(|f| f.id == uuid) {
            return self.try_send(WebEvent::TextFile(file.clone()));
        }
        match self.forward_request(&WebRequest::FileQuery {
            file_id: uuid.to_string(),
        }) {
            Ok(()) => return false,
            Err(ClientError::NoLocationError) => {
                self.broadcast();
                self.pending_request = Some(WebRequest::FileQuery {
                    file_id: uuid.to_string(),
                });
            }
            Err(e) => {
                eprintln!("Error forwarding request: {e}");
                return false;
            }
        }
        false
    }

    fn handle_get_media_files(&self) -> bool {
        let media: HashSet<_> = self.cached_files.values().flatten().cloned().collect();
        self.try_send(WebEvent::MediaFiles(media.into_iter().collect()))
    }

    fn handle_get_media_file(&mut self, media_id: Uuid, location: NodeId) -> bool {
        for v in self.cached_files.values() {
            if let Some(media) = v.iter().find(|m| m.id == media_id) {
                return self.try_send(WebEvent::MediaFile(media.clone()));
            }
        }
        if let Ok(req) = serde_json::to_vec(&WebRequest::MediaQuery {
            media_id: media_id.to_string(),
        }) {
            let _ = self.routing_handler.send_message(&req, location, None);
        }
        false
    }
}

impl Processor for WebBrowser {
    fn controller_recv(&self) -> &Receiver<Box<dyn Command>> {
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

    fn handle_command(&mut self, cmd: Box<dyn Command>) -> bool {

        let cmd = cmd.into_any();
        if let Some(cmd) = cmd.downcast_ref::<WebCommand>() {
            match cmd {
                WebCommand::GetCachedFiles => self.handle_get_cached_files(),
                WebCommand::GetFile(uuid) => self.handle_get_file(*uuid),
                WebCommand::GetTextFiles => self.handle_get_text_files(),
                WebCommand::GetTextFile(uuid) => self.handle_get_text_file(*uuid),
                WebCommand::GetMediaFiles => self.handle_get_media_files(),
                WebCommand::GetMediaFile { media_id, location } => {
                    self.handle_get_media_file(*media_id, *location)
                }
                _ => {
                    eprintln!("Unsupported command: {cmd:?}");
                    todo!()
                }
            }
        } else if let Some(cmd) = cmd.downcast_ref::<NodeCommand>() {
            match cmd {
                NodeCommand::AddSender(node_id, sender) => {
                    self.routing_handler.add_neighbor(*node_id, sender.clone());
                    return false;
                }
                NodeCommand::RemoveSender(node_id) => {
                    self.routing_handler.remove_neighbor(*node_id);
                    return false;
                }
                NodeCommand::Shutdown => return true,
            }
        } else {
            false
        }
    }

    fn handle_msg(&mut self, msg: Vec<u8>, from: NodeId, session_id: u64) {
        if let Ok(msg) = serde_json::from_slice::<WebResponse>(&msg) {
            match msg {
                WebResponse::ServerType { server_type } => {
                    if matches!(server_type, ServerType::TextServer) {
                        self.text_servers.insert(from, vec![]);
                        let _ = self.forward_request(&WebRequest::TextFilesListQuery);
                    }
                }
                WebResponse::TextFilesList { files } => {
                    self.set_files_list(from, files);
                    if let Some(req) = self.pending_request.take() {
                        match self.forward_request(&req) {
                            Ok(()) => {}
                            Err(ClientError::NoLocationError) => {
                                self.pending_request = Some(req);
                            }
                            Err(e) => eprintln!("Error forwarding request: {e}"),
                        }
                    }
                }
                WebResponse::TextFile { file_data } => {
                    if let Ok(file) = serde_json::from_slice::<TextFile>(&file_data) {
                        self.manage_text_file(file, session_id);
                    }
                }
                WebResponse::MediaFile { media_data } => {
                    if let Ok(mediafile) = serde_json::from_slice::<MediaFile>(&media_data) {
                        // check if all media files are present, if yes send to controller
                        self.manage_media_file(mediafile);
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
    use common::types::{MediaFile, MediaReference, ServerType, TextFile, WebResponse};
    use crossbeam::channel::unbounded;

    fn create_test_web_browser() -> WebBrowser {
        let (_controller_send, controller_recv) = unbounded();
        let (event_send, _event_recv) = unbounded();
        let (_, packet_recv) = unbounded();
        let neighbors = HashMap::new();

        WebBrowser::new(1, neighbors, packet_recv, controller_recv, event_send)
    }

    #[test]
    /// Tests `ServerType` response handling (text server being added to `HashSet`)
    fn test_server_type_identification() {
        let mut browser = create_test_web_browser();

        let response = WebResponse::ServerType {
            server_type: ServerType::TextServer,
        };
        let serialized = serde_json::to_vec(&response).unwrap();
        browser.handle_msg(serialized, 5, 100);

        assert!(browser.text_servers.contains_key(&5));
    }

    #[test]
    /// Tests `TextFilesList` response handling (server files list being added to `HashSet`)
    fn test_text_files_list_handling() {
        let mut browser = create_test_web_browser();

        let files = vec![
            "file1-id:Article 1".to_string(),
            "file2-id:Article 2".to_string(),
        ];
        let response = WebResponse::TextFilesList { files };
        let serialized = serde_json::to_vec(&response).unwrap();
        browser.handle_msg(serialized, 5, 101);

        let server_files = browser.get_list_files_by_id(5).unwrap();
        assert_eq!(server_files.len(), 2);
    }

    #[test]
    /// Tests `TextFile` handling and caching
    fn test_text_file_caching_with_media_refs() {
        let mut browser = create_test_web_browser();

        let media_ref = MediaReference::new(6);
        let text_file = TextFile::new(
            "Article with Media".to_string(),
            "Content with media ref".to_string(),
            vec![media_ref],
        );
        let serialized_file = serde_json::to_vec(&text_file).unwrap();
        let response = WebResponse::TextFile {
            file_data: serialized_file,
        };
        let serialized = serde_json::to_vec(&response).unwrap();
        browser.handle_msg(serialized, 5, 102);

        assert!(browser.cached_files.contains_key(&text_file));
        let cached_files = browser.get_text_files();
        assert_eq!(cached_files.len(), 1);
    }

    #[test]
    /// Tests association between `TextFile` and `MediaFile` in `File`
    fn test_media_file_association() {
        let mut browser = create_test_web_browser();

        let media_ref = MediaReference::new(6);
        let text_file = TextFile::new(
            "Article".to_string(),
            "Content".to_string(),
            vec![media_ref.clone()],
        );
        browser.cached_files.insert(text_file.clone(), vec![]);
        let media_file = MediaFile {
            id: media_ref.id,
            title: "Test Image".to_string(),
            content: vec![vec![1, 2, 3, 4]],
        };
        let serialized_media = serde_json::to_vec(&media_file).unwrap();
        let response = WebResponse::MediaFile {
            media_data: serialized_media,
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
            let should_not_continue = browser.handle_command(Box::new(cmd));
            assert!(should_not_continue);
        }
    }
}
