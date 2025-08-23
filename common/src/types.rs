use std::any::Any;
use std::{collections::HashMap, str::FromStr};
use std::fmt::Display;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use wg_internal::{network::NodeId, packet::Packet};
use crossbeam_channel::Sender;
use uuid::Uuid;
pub type Bytes = Vec<u8>;

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct MediaReference {
    location: NodeId,
    pub id: Uuid
}

impl MediaReference {
    #[must_use]
    pub fn new(location: NodeId) -> Self {
        Self {
            location,
            id: Uuid::new_v4()
        }
    }

    #[must_use]
    pub fn get_location(&self) -> NodeId {
        self.location
    }
}

impl Display for MediaReference {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}/{}", self.location, self.id)
    }
}

impl FromStr for MediaReference {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (location, id) = value.split_at({
            if let Some(c) = value.chars().position(|c| c == '/') {
                c
            } else {
                return Err(anyhow!("Cannot parse media reference"))
            }
        });
        Ok(Self { location: u8::from_str(location)?, id: Uuid::from_str(id)? })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct TextFile{
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub media_refs: Vec<MediaReference>
}


impl TextFile {
    #[must_use]
    pub fn new(title: String, content: String, media_refs: Vec<MediaReference>) -> Self {
        Self {
            title,
            id: Uuid::new_v4(),
            content,
            media_refs
        }
    }

    #[must_use]
    pub fn get_refs(&self) -> Vec<MediaReference> {
        self.media_refs.clone()
    }


    #[must_use]
    pub fn get_media_ids(&self) -> Vec<Uuid> {
        self.media_refs.iter().map(|m| m.id).collect()
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]

pub struct MediaFile {
    pub id: Uuid,
    pub title: String,
    pub content: Vec<Bytes>,
}

impl MediaFile {
    #[must_use]
    pub fn new(title: String, content: Vec<Bytes>) -> Self {
        Self {
            id: Uuid::new_v4(),
            title,
            content,
        }
    }

    #[must_use]
    pub fn from_u8(filename: String, data: &[u8]) -> Self {
        let chunk_size = 1024;
        let content: Vec<Bytes> = data
            .chunks(chunk_size)
            .map(<[u8]>::to_vec)
            .collect();

        Self::new(filename, content)
    }

    #[must_use]
    pub fn get_title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn get_content(&self) -> &Vec<Bytes> {
        &self.content
    }

    #[must_use]
    pub fn get_size(&self) -> usize {
        self.content.iter().map(Vec::len).sum()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct File {
    pub id: Uuid,
    pub text_file: TextFile,
    media_files: Vec<MediaFile>
}


impl File {
    #[must_use]
    pub fn new(text_file: TextFile, media_files: Vec<MediaFile>) -> Self {
        Self {
            id: text_file.id,
            text_file,
            media_files
        }
    }
}


#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "request_type")]
pub enum WebRequest {
    #[serde(rename = "server_type?")]
    ServerTypeQuery,

    #[serde(rename = "files_list?")]
    TextFilesListQuery,

    #[serde(rename = "file?")]
    FileQuery { file_id: String },

    #[serde(rename = "media?")]
    MediaQuery { media_id: String },
}

impl WebRequest {
    #[must_use]
    pub fn get_file_id(&self) -> Option<String> {
        match self {
            Self::FileQuery { file_id} => Some(file_id.clone()),
            Self::MediaQuery { media_id } => Some(media_id.clone()),
            _ => None
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "response_type")]
pub enum WebResponse {
    #[serde(rename = "server_type!")]
    ServerType { server_type: ServerType },

    #[serde(rename = "files_list!")]
    TextFilesList { files: Vec<String> },

    #[serde(rename = "file!")]
    TextFile { file_data: Vec<u8> },

    #[serde(rename = "media!")]
    MediaFile { media_data: Vec<u8> },

    #[serde(rename = "error_requested_not_found!")]
    ErrorFileNotFound(Uuid),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "request_type")]
pub enum ChatRequest {
    #[serde(rename = "server_type?")]
    ServerTypeQuery,

    #[serde(rename = "registration_to_chat")]
    RegistrationToChat { client_id: NodeId },

    #[serde(rename = "client_list?")]
    ClientListQuery,

    #[serde(rename = "message_for?")]
    MessageFor { client_id: NodeId, message: String },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "response_type")]
pub enum ChatResponse {
    #[serde(rename = "server_type!")]
    ServerType { server_type: ServerType },

    #[serde(rename = "client_list!")]
    ClientList { list_of_client_ids: Vec<NodeId> },

    #[serde(rename = "message_from!")]
    MessageFrom { client_id: NodeId, message: String },

    #[serde(rename = "error_wrong_client_id!")]
    ErrorWrongClientId,

    // Custom response for successful registration
    #[serde(rename = "registration_success")]
    RegistrationSuccess,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub from: NodeId,
    pub to: NodeId,
    pub text: String,
}

impl Message {
    #[must_use]
    pub fn new(from: NodeId, to: NodeId, text: String) -> Self {
        Message { from, to, text }
    }
}

pub trait Command: Send {
    fn as_any(&self) -> &dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;

}

impl<T: 'static + Send> Command for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}


pub trait Event: Send {
    fn as_any(&self) -> &dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<T: 'static + Send> Event for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

#[derive(Debug, Clone)]
pub enum ChatCommand {
    GetChatsHistory,
    GetRegisteredClients,
    SendMessage(Message)
}


#[derive(Debug, Clone)]
pub enum ChatEvent {
    ChatHistory(HashMap<NodeId, Vec<Message>>),
    RegisteredClients(Vec<NodeId>),
    MessageSent,
    MessageReceived(Message)
}

#[derive(Debug, Clone)]
pub enum WebCommand {
    GetCachedFiles,
    GetFile(Uuid),
    GetTextFiles,
    GetTextFile(Uuid),
    GetMediaFiles,
    GetMediaFile { media_id: Uuid, location: NodeId },
    AddTextFile(TextFile),
    AddTextFileFromPath(String),
    AddMediaFile(MediaFile),
    AddMediaFileFromPath(String),
    RemoveTextFile(Uuid),
    RemoveMediaFile(Uuid),
}


#[derive(Debug, Clone)]
pub enum WebEvent {
    CachedFiles(Vec<File>),
    File(File),
    TextFiles(Vec<TextFile>),
    TextFile(TextFile),
    MediaFiles(Vec<MediaFile>),
    MediaFile(MediaFile),
    FileNotFound(Uuid),
    TextFileAdded(Uuid),
    MediaFileAdded(Uuid),
    TextFileRemoved(Uuid),
    MediaFileRemoved(Uuid),
    FileOperationError(String),
}


#[derive(Debug, Clone)]
pub enum NodeEvent {
    PacketSent(Packet),
    FloodStarted(u64, NodeId),
    NodeRemoved(NodeId)
}

#[derive(Debug, Clone)]
pub enum NodeCommand {
    AddSender(NodeId, Sender<Packet>),
    RemoveSender(NodeId),
    Shutdown,
}


impl NodeCommand {
    #[must_use]
    pub fn as_add_sender(self) -> Option<(NodeId, Sender<Packet>)> {
        match self {
            NodeCommand::AddSender(id, sender) => Some((id, sender)),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_add_sender(&self) -> bool {
        matches!(self, Self::AddSender(_, _))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClientType {
    ChatClient,
    WebBrowser,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ServerType {
    ChatServer,
    TextServer,
    MediaServer,
}
