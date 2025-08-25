use std::collections::HashMap;
use crossbeam::channel::{Receiver, Sender};
use uuid::Uuid;
use wg_internal::network::NodeId;
use wg_internal::packet::{NodeType, Packet};
use common::{FragmentAssembler, RoutingHandler};
use common::packet_processor::Processor;
use common::types::{Command, Event, MediaFile, NodeCommand, NodeEvent, ServerType, WebCommand, WebEvent, WebRequest, WebResponse};
use common::file_conversion;

pub struct MediaServer {
    routing_handler: RoutingHandler,
    controller_recv: Receiver<Box<dyn Command>>,
    controller_send: Sender<Box<dyn Event>>,
    packet_recv: Receiver<Packet>,
    id: NodeId,
    assembler: FragmentAssembler,
    stored_media: HashMap<Uuid, MediaFile>,
}

impl MediaServer {
    #[must_use]
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
            stored_media: HashMap::new(),
        }
    }

    fn get_media_by_id(&self, media_id: Uuid) -> Option<&MediaFile> {
        self.stored_media.get(&media_id)
    }

    pub fn add_media_file(&mut self, media_file: MediaFile) {
        self.stored_media.insert(media_file.id, media_file);
    }

    pub fn remove_media_file(&mut self, media_id: Uuid) -> Option<MediaFile> {
        self.stored_media.remove(&media_id)
    }

    fn get_all_media_files(&self) -> Vec<MediaFile> {
        self.stored_media.values().cloned().collect()
    }

    fn get_media_list(&self) -> Vec<String> {
        self.stored_media
            .values()
            .map(|file| format!("{}:{}", file.id, file.title))
            .collect()
    }
}

impl Processor for MediaServer {
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
        let _ = self.controller_send.send(Box::new(NodeEvent::MessageReceived {
            notification_from: self.id,
            from
        }));
        if let Ok(msg) = serde_json::from_slice::<WebRequest>(&msg) {
            match msg {
                WebRequest::ServerTypeQuery => {
                    let _ = self.controller_send.send(Box::new(NodeEvent::ServerTypeQueried {
                        notification_from: self.id,
                        from
                    }));
                    if let Ok(res) = serde_json::to_vec(&WebResponse::ServerType { server_type: ServerType::MediaServer }) {
                        let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                        let _ = self.controller_send.send(Box::new(NodeEvent::MessageSent {
                            notification_from: self.id,
                            to: from
                        }));
                    }
                }
                WebRequest::MediaQuery { media_id } => {
                    let _ = self.controller_send.send(Box::new(WebEvent::FileRequested {
                        notification_from: self.id,
                        from,
                        uuid: media_id.clone(),
                    }));
                    match Uuid::parse_str(&media_id) {
                        Ok(uuid) => {
                            if let Some(media_file) = self.get_media_by_id(uuid) {
                                if let Ok(serialized_media) = serde_json::to_vec(media_file)
                                    && let Ok(res) = serde_json::to_vec(&WebResponse::MediaFile {
                                        media_data: serialized_media
                                    }) {
                                        let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                                        let _ = self.controller_send.send(Box::new(NodeEvent::MessageSent {
                                            notification_from: self.id,
                                            to: from
                                        }));
                                        let _ = self.controller_send.send(Box::new(WebEvent::FileServed {
                                            notification_from: self.id,
                                            file: media_id.clone(),
                                        }));
                                }
                            } else if let Ok(res) = serde_json::to_vec(&WebResponse::ErrorFileNotFound(uuid)) {
                                    let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                                    let _ = self.controller_send.send(Box::new(NodeEvent::MessageSent {
                                        notification_from: self.id,
                                        to: from
                                    }));
                                }
                        }
                        Err(_) => {
                            if let Ok(res) = serde_json::to_vec(&WebResponse::BadUuid(media_id.clone())) {
                                let _ = self.routing_handler.send_message(&res, from, Some(session_id));
                                let _ = self.controller_send.send(Box::new(NodeEvent::MessageSent {
                                    notification_from: self.id,
                                    to: from
                                }));
                                let _ = self.controller_send.send(Box::new(WebEvent::BadUuid {
                                    notification_from: self.id,
                                    from,
                                    uuid: media_id,
                                }));
                            }
                        }
                    }
                }
                _ => {}
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
                WebCommand::GetMediaFiles => {
                    let media_files = self.get_all_media_files();
                    if self.controller_send
                        .send(Box::new(WebEvent::MediaFiles {
                            notification_from: self.id,
                            files: media_files,
                        }))
                        .is_err()
                    {
                        return true;
                    }
                }
                WebCommand::GetMediaFile{media_id, location: _location} => {
                    if let Some(media_file) = self.get_media_by_id(*media_id)
                        && self.controller_send
                            .send(Box::new(WebEvent::MediaFile {
                                notification_from: self.id,
                                file: media_file.clone(),
                            }))
                            .is_err()
                        {
                            return true;
                        }
                }
                WebCommand::AddMediaFile(media_file) => {
                    let file_id = media_file.id;
                    self.add_media_file(media_file.clone());

                    if self.controller_send
                        .send(Box::new(WebEvent::MediaFileAdded {
                            notification_from: self.id,
                            uuid: file_id,
                        }))
                        .is_err()
                    {
                        return true;
                    }
                }
                WebCommand::AddMediaFileFromPath(file_path) => {
                    match file_conversion::file_to_media_file(file_path) {
                        Ok(media_file) => {
                            let file_id = media_file.id;
                            self.add_media_file(media_file);

                            if self.controller_send
                                .send(Box::new(WebEvent::MediaFileAdded {
                                    notification_from: self.id,
                                    uuid: file_id,
                                }))
                                .is_err()
                            {
                                return true;
                            }
                        }
                        Err(conversion_error) => {
                            if self.controller_send
                                .send(Box::new(WebEvent::FileOperationError {
                                    notification_from: self.id,
                                    msg: format!("Failed to convert file {file_path}: {conversion_error}"),
                                }))
                                .is_err()
                            {
                                return true;
                            }
                        }
                    }
                }
                WebCommand::RemoveMediaFile(uuid) => {
                    if let Some(removed_file) = self.remove_media_file(*uuid) {
                        if self.controller_send
                            .send(Box::new(WebEvent::MediaFileRemoved {
                                notification_from: self.id,
                                uuid: removed_file.id
                            }))
                            .is_err()
                        {
                            return true;
                        }
                    } else if self.controller_send
                            .send(Box::new(WebEvent::FileOperationError {
                                notification_from: self.id,
                                msg: format!("Media file with ID {uuid} not found"),
                            }))
                            .is_err()
                        {
                            return true;
                        }
                }
                _ => {}
            }
        }
        false
    }
}

#[cfg(test)]
mod media_server_tests {
    use super::*;
    use crossbeam::channel::unbounded;

    #[test]
    fn test_media_server_creation() {
        let (_controller_send, controller_recv) = unbounded();
        let (event_send, _event_recv) = unbounded::<Box<dyn Event>>();
        let (_, packet_recv) = unbounded();

        let server = MediaServer::new(1, HashMap::new(), packet_recv, controller_recv, event_send);

        assert_eq!(server.id, 1);
        assert!(server.stored_media.is_empty());
    }

    #[test]
    fn test_get_media_list() {
        let (_controller_send, controller_recv) = unbounded();
        let (event_send, _event_recv) = unbounded::<Box<dyn Event>>();
        let (_, packet_recv) = unbounded();

        let mut server = MediaServer::new(1, HashMap::new(), packet_recv, controller_recv, event_send);
        let test_media = MediaFile::new(
            "test_image.png".to_string(),
            vec![vec![0x89, 0x50, 0x4E, 0x47]]
        );
        server.add_media_file(test_media);
        let media_list = server.get_media_list();

        assert!(!media_list.is_empty());
    }

    #[test]
    fn test_add_and_retrieve_media() {
        let (_controller_send, controller_recv) = unbounded();
        let (event_send, _event_recv) = unbounded::<Box<dyn Event>>();
        let (_, packet_recv) = unbounded();

        let mut server = MediaServer::new(1, HashMap::new(), packet_recv, controller_recv, event_send);

        let test_media = MediaFile::new(
            "test_image.png".to_string(),
            vec![vec![0x89, 0x50, 0x4E, 0x47]]
        );
        let media_id = test_media.id;

        server.add_media_file(test_media);

        let retrieved = server.get_media_by_id(media_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().get_title(), "test_image.png");
    }

    #[test]
    fn test_media_file_creation() {
        let content = vec![
            vec![0x00, 0x01, 0x02],
            vec![0x03, 0x04, 0x05],
        ];
        let media = MediaFile::new("test.bin".to_string(), content);

        assert_eq!(media.get_title(), "test.bin");
        assert_eq!(media.get_size(), 6);
        assert_eq!(media.get_content().len(), 2);
    }
}
