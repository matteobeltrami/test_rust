use std::any::Any;
use std::collections::{HashMap};
use crossbeam::channel::{Receiver, Sender};
use uuid::{Uuid};
use wg_internal::network::NodeId;
use wg_internal::packet::{NodeType, Packet};
use common::{FragmentAssembler, RoutingHandler};
use common::packet_processor::Processor;
use common::types::{file_conversion, File, NodeCommand, ServerType, TextFile, WebCommand, WebEvent, WebRequest, WebResponse};

pub struct TextServer {
    routing_handler: RoutingHandler,
    controller_recv: Receiver<Box<dyn Any>>,
    controller_send: Sender<Box<dyn Any>>,
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
        controller_recv: Receiver<Box<dyn Any>>,
        controller_send: Sender<Box<dyn Any>>
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

    fn handle_command(&mut self, cmd: Box<dyn Any>) -> bool {
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
                WebCommand::GetMediaFile(uuid) => {
                    if self.controller_send
                        .send(Box::new(WebEvent::FileNotFound(*uuid)))
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
                    match file_conversion::file_to_text_file(file_path) {
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
mod tests {
    use super::*;
    use crossbeam::channel::unbounded;

    #[test]
    fn test_text_server_creation() {
        let (controller_send, controller_recv) = unbounded();
        let (_, packet_recv) = unbounded();

        let server = TextServer::new(1, HashMap::new(), packet_recv, controller_recv, controller_send);

        assert_eq!(server.id, 1);
        assert!(server.stored_files.is_empty());
    }

    #[test]
    fn test_get_files_list() {
        let (controller_send, controller_recv) = unbounded();
        let (_, packet_recv) = unbounded();

        let mut server = TextServer::new(1, HashMap::new(), packet_recv, controller_recv, controller_send);
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
    fn test_add_and_retrieve_file() {
        let (controller_send, controller_recv) = unbounded();
        let (_, packet_recv) = unbounded();

        let mut server = TextServer::new(1, HashMap::new(), packet_recv, controller_recv, controller_send);

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
}