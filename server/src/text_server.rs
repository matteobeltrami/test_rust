use std::collections::HashMap;
use crossbeam::channel::{Receiver, Sender};
use uuid::Uuid;
use wg_internal::network::NodeId;
use wg_internal::packet::{NodeType, Packet};
use common::{FragmentAssembler, RoutingHandler};
use common::packet_processor::Processor;
use common::types::{Command, Event, File, NodeCommand, ServerType, TextFile, WebCommand, WebEvent, WebRequest, WebResponse};
use common::file_conversion;

pub struct TextServer {
    routing_handler: RoutingHandler,
    controller_recv: Receiver<Box<dyn Command>>,
    controller_send: Sender<Box<dyn Event>>,
    packet_recv: Receiver<Packet>,
    id: NodeId,
    assembler: FragmentAssembler,
    stored_files: HashMap<Uuid, TextFile>,
}

impl TextServer {
    pub fn new(
        id: NodeId,
        neighbors: HashMap<NodeId, Sender<Packet>>,
        packet_recv: Receiver<Packet>,
        controller_recv: Receiver<Box<dyn Command>>,
        controller_send: Sender<Box<dyn Event>>
    ) -> Self {
        let router = RoutingHandler::new(id, NodeType::Server, neighbors, controller_send.clone());
        Self {
            routing_handler: router,
            controller_recv,
            controller_send,
            packet_recv,
            id,
            assembler: FragmentAssembler::default(),
            stored_files: HashMap::new(),
        }
    }

    fn get_files_list(&self) -> Vec<String> {
        self.stored_files
            .values()
            .map(|file| format!("{}:{}", file.id, file.title))
            .collect()
    }

    fn get_file_by_id(&self, file_id: Uuid) -> Option<&TextFile> {
        self.stored_files.get(&file_id)
    }

    fn add_text_file(&mut self, text_file: TextFile) {
        self.stored_files.insert(text_file.id, text_file);
    }

    pub fn remove_text_file(&mut self, file_id: Uuid) -> Option<TextFile> {
        self.stored_files.remove(&file_id)
    }

    fn get_all_text_files(&self) -> Vec<TextFile> {
        self.stored_files.values().cloned().collect()
    }
}

impl Processor for TextServer {
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

    fn handle_msg(&mut self, msg: Vec<u8>, from: NodeId, session_id: u64) {
        if let Ok(msg) = serde_json::from_slice::<WebRequest>(&msg) {
            match msg {
                WebRequest::ServerTypeQuery => {
                    if let Ok(res) = serde_json::to_vec(&WebResponse::ServerType { server_type: ServerType::TextServer }) {
                        let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                    }
                }
                WebRequest::TextFilesListQuery => {
                    let files_list = self.get_files_list();
                    if let Ok(res) = serde_json::to_vec(&WebResponse::TextFilesList {files: files_list}) {
                        let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                    }
                }
                WebRequest::FileQuery { file_id } => {
                    match Uuid::parse_str(&file_id) {
                        Ok(uuid) => {
                            if let Some(text_file) = self.get_file_by_id(uuid) {
                                if let Ok(serialized_file) = serde_json::to_vec(text_file) {
                                    if let Ok(res) = serde_json::to_vec(&WebResponse::TextFile { file_data: serialized_file }) {
                                        let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                                    }
                                }
                            } else {
                                if let Ok(res) = serde_json::to_vec(&WebResponse::ErrorFileNotFound(uuid)) {
                                    let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                                }
                            }
                        }
                        Err(_) => {
                            // eprintln!("Invalid UUID format in file query: {}", file_id);
                            todo!()
                        }
                    }
                }
                WebRequest::MediaQuery { .. } => {
                    // eprintln!("Text server received media query - this should be handled by media server");
                    todo!();
                }
            }
        }
    }

    fn handle_command(&mut self, cmd: Box<dyn Command>) -> bool {
        let cmd = cmd.into_any();
        if let Some(cmd) = cmd.downcast_ref::<NodeCommand>() {
            match cmd {
                NodeCommand::AddSender(node_id, sender) => self.routing_handler.add_neighbor(*node_id, sender.clone()),
                NodeCommand::RemoveSender(node_id) => self.routing_handler.remove_neighbor(*node_id),
                NodeCommand::Shutdown => return true
            }
        }  else if let Some(cmd) = cmd.downcast_ref::<WebCommand>() {
            match cmd {
                WebCommand::GetCachedFiles => {
                    let files = self.get_all_text_files();
                    let files_as_full_files = files.into_iter().map(|tf| {
                        File::new(tf, vec![]) // No media files in text server
                    }).collect();

                    if self.controller_send
                        .send(Box::new(WebEvent::CachedFiles(files_as_full_files)))
                        .is_err()
                    {
                        return true;
                    }
                }
                WebCommand::GetFile(uuid) => {
                    if let Some(text_file) = self.get_file_by_id(*uuid) {
                        let file = File::new(text_file.clone(), vec![]);
                        if self.controller_send
                            .send(Box::new(WebEvent::File(file)))
                            .is_err()
                        {
                            return true;
                        }
                    }
                }
                WebCommand::GetTextFiles => {
                    let text_files = self.get_all_text_files();
                    if self.controller_send
                        .send(Box::new(WebEvent::TextFiles(text_files)))
                        .is_err()
                    {
                        return true;
                    }
                }
                WebCommand::GetTextFile(uuid) => {
                    if let Some(text_file) = self.get_file_by_id(*uuid) {
                        if self.controller_send
                            .send(Box::new(WebEvent::TextFile(text_file.clone())))
                            .is_err()
                        {
                            return true;
                        }
                    }
                }
                WebCommand::GetMediaFiles => {
                    if self.controller_send
                        .send(Box::new(WebEvent::MediaFiles(vec![])))
                        .is_err()
                    {
                        return true;
                    }
                }
                WebCommand::GetMediaFile{media_id,location: _location} => {
                    if self.controller_send
                        .send(Box::new(WebEvent::FileNotFound(*media_id)))
                        .is_err()
                    {
                        return true;
                    }
                    todo!()
                }
                WebCommand::AddTextFile(text_file) => {
                    let file_id = text_file.id;
                    self.add_text_file(text_file.clone());

                    if self.controller_send
                        .send(Box::new(WebEvent::TextFileAdded(file_id)))
                        .is_err()
                    {
                        return true;
                    }
                }
                WebCommand::AddTextFileFromPath(file_path) => {
                    match file_conversion::file_to_text_file(&file_path) {
                        Ok(text_file) => {
                            let file_id = text_file.id;
                            self.add_text_file(text_file);

                            if self.controller_send
                                .send(Box::new(WebEvent::TextFileAdded(file_id)))
                                .is_err()
                            {
                                return true;
                            }
                        }
                        Err(conversion_error) => {
                            if self.controller_send
                                .send(Box::new(WebEvent::FileOperationError(
                                    format!("Failed to convert text file {}: {}", file_path, conversion_error)
                                )))
                                .is_err()
                            {
                                return true;
                            }
                        }
                    }
                }
                WebCommand::RemoveTextFile(uuid) => {
                    if let Some(removed_file) = self.remove_text_file(*uuid) {
                        if self.controller_send
                            .send(Box::new(WebEvent::TextFileRemoved(removed_file.id)))
                            .is_err()
                        {
                            return true;
                        }
                    } else {
                        if self.controller_send
                            .send(Box::new(WebEvent::FileOperationError(
                                format!("Text file with ID {} not found", uuid)
                            )))
                            .is_err()
                        {
                            return true;
                        }
                    }
                }
                WebCommand::AddMediaFile(_) | WebCommand::AddMediaFileFromPath(_) | WebCommand::RemoveMediaFile(_) => {
                    if self.controller_send
                        .send(Box::new(WebEvent::FileOperationError(
                            "Text server cannot store media files".to_string()
                        )))
                        .is_err()
                    {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod text_server_tests {
    use super::*;
    use crossbeam::channel::unbounded;
    use common::types::MediaReference;

    fn create_test_text_server() -> TextServer {
        let (controller_send, controller_recv) = unbounded();
        let (event_send, _event_recv) = unbounded::<Box<dyn Event>>();
        let (_, packet_recv) = unbounded();
        TextServer::new(1, HashMap::new(), packet_recv, controller_recv, event_send)
    }

    #[test]
    /// Tests the server creation
    fn test_text_server_creation() {
        let server = create_test_text_server();

        assert_eq!(server.id, 1);
        assert!(server.stored_files.is_empty());
    }

    #[test]
    /// Tests adding text files and retrieving the files list
    fn test_get_files_list() {
        let mut server = create_test_text_server();

        let test_file = TextFile::new(
            "Test File".to_string(),
            "This is a test".to_string(),
            vec![]
        );
        let file_id = test_file.id;
        let title = test_file.clone().title;
        server.add_text_file(test_file);

        let files_list = server.get_files_list();

        assert!(!files_list.is_empty());
        for entry in &files_list {
            assert_eq!(*entry, format!("{}:{}", file_id, title));
        }
    }

    #[test]
    /// Tests adding and retrieving text files
    fn test_add_and_retrieve_file() {
        let mut server = create_test_text_server();

        let test_file = TextFile::new(
            "Test File".to_string(),
            "This is a test file content.".to_string(),
            vec![]
        );
        let file_id = test_file.id;
        let title = test_file.clone().title;

        server.add_text_file(test_file);

        let retrieved = server.get_file_by_id(file_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, title);
    }

    #[test]
    /// Tests ServerTypeQuery, TextFilesListQuery, FileQuery (correct and incorrect uuid) web requests handling
    fn test_web_request_format_validation() {
        let mut server = create_test_text_server();

        let media_ref = MediaReference::new(5);
        let text_file = TextFile::new(
            "Test Article".to_string(),
            "This is a test article with media reference: {}".to_string(),
            vec![media_ref]
        );
        let file_id = text_file.id;
        server.add_text_file(text_file);

        let request = WebRequest::ServerTypeQuery;
        let serialized = serde_json::to_vec(&request).unwrap();
        server.handle_msg(serialized, 2, 100);

        let list_request = WebRequest::TextFilesListQuery;
        let serialized = serde_json::to_vec(&list_request).unwrap();
        server.handle_msg(serialized, 2, 101);

        let file_request = WebRequest::FileQuery {
            file_id: file_id.to_string()
        };
        let serialized = serde_json::to_vec(&file_request).unwrap();
        server.handle_msg(serialized, 2, 102);

        let invalid_request = WebRequest::FileQuery {
            file_id: "invalid-uuid".to_string()
        };
        let serialized = serde_json::to_vec(&invalid_request).unwrap();

        let nonexistent_request = WebRequest::FileQuery {
            file_id: Uuid::new_v4().to_string()
        };
        let serialized = serde_json::to_vec(&nonexistent_request).unwrap();
        server.handle_msg(serialized, 2, 103);
    }

    #[test]
    /// Tests the handling of a large file (~12KB)
    fn test_large_file_content_handling() {
        let mut server = create_test_text_server();

        let large_content = "Lorem ipsum ".repeat(1000);
        let large_file = TextFile::new(
            "Large Article".to_string(),
            large_content.clone(),
            vec![]
        );
        server.add_text_file(large_file.clone());

        let retrieved = server.get_file_by_id(large_file.id).unwrap();
        assert_eq!(retrieved.content.len(), large_content.len());
        assert_eq!(retrieved.content, large_content);
    }

    #[test]
    /// Tests the formatting of the files list
    fn test_files_list_format() {
        let mut server = create_test_text_server();

        let file1 = TextFile::new("Article 1".to_string(), "Content 1".to_string(), vec![]);
        let file2 = TextFile::new("Article 2".to_string(), "Content 2".to_string(), vec![]);
        let id1 = file1.id;
        let id2 = file2.id;
        server.add_text_file(file1);
        server.add_text_file(file2);

        let files_list = server.get_files_list();
        assert_eq!(files_list.len(), 2);
        assert!(files_list.contains(&format!("{}:Article 1", id1)));
        assert!(files_list.contains(&format!("{}:Article 2", id2)));
    }

    #[test]
    /// Tests AddTextFile and RemoveTextFile commands
    fn test_command_handling() {
        let mut server = create_test_text_server();

        let test_file = TextFile::new(
            "Command Test".to_string(),
            "Added via command".to_string(),
            vec![]
        );
        let file_id = test_file.id;

        let add_command = WebCommand::AddTextFile(test_file);
        let should_not_continue = server.handle_command(Box::new(add_command));
        assert!(should_not_continue);
        assert!(server.get_file_by_id(file_id).is_some());

        let remove_command = WebCommand::RemoveTextFile(file_id);
        let should_not_continue = server.handle_command(Box::new(remove_command));
        assert!(should_not_continue);
        assert!(server.get_file_by_id(file_id).is_none());
    }
}
