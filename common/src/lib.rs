
use strum_macros::EnumString;
use num_enum::{FromPrimitive, IntoPrimitive};

pub mod util;
pub mod logger;


pub const API_INFO: [u8; 6] = [b'R', b'M', b'6', b'4', 0x00, 0x00];


#[derive(Debug, PartialEq)]
pub enum PacketError {
    Empty,
    UnexpectedLength
}
use PacketError::*;
use crate::Packet::Unknown;

pub const ID_PING: u8 = 0x01;
pub const ID_PONG: u8 = 0x02;
pub const ID_INFO_REQ: u8 = 0x03;
pub const ID_INFO_RES: u8 = 0x04;
pub const ID_QUEUE_REQ: u8 = 0x05;
pub const ID_QUEUE_RES: u8 = 0x06;
pub const ID_IMAGE_REQ: u8 = 0x07;
pub const ID_IMAGE_RES: u8 = 0x08;
pub const ID_REQ_DENIED: u8 = 0xFE;
pub const ID_UNKNOWN: u8 = 0xFF;


#[derive(Copy, Clone, Debug, PartialEq, FromPrimitive, IntoPrimitive, EnumString)]
#[repr(u8)]
pub enum Capability {
    LivePlayback = 0x01,
    AudioRecording = 0x02,
    InputHandling = 0x03,
    
    #[num_enum(default)]
    Invalid = 0x00,
}
impl Default for Capability {
    fn default() -> Self {
        Capability::Invalid
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct ServerInfo {
    pub header: [u8; 4],
    pub version: u16,
    pub capabilities: Vec<Capability>,
}
impl ServerInfo {
    pub fn serialize(&self) -> Vec<u8> {
        let mut raw = vec![];
        raw.copy_from_slice(&self.header);
        raw.copy_from_slice(&self.version.to_be_bytes());
        for cap in &self.capabilities {
            raw.push((*cap).into());
        }
        
        raw
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Packet {
    Ping,
    Pong,
    InfoRequest,
    InfoResponse(ServerInfo),
    QueueRequest,
    QueueResponse(u32),
    ImageRequest,
    ImageResponse(Vec<u8>),
    RequestDenied,
    Unknown(Vec<u8>),
}
use Packet::*;

impl Packet {
    pub fn deserialize(data: &[u8]) -> Result<Packet, PacketError> {
        if data.is_empty() { return Err(Empty) }
        
        match data[0] {
            ID_PING => Ok(Ping),
            ID_PONG => Ok(Pong),
            ID_INFO_REQ => Ok(InfoRequest),
            ID_INFO_RES => {
                if data.len() < 7 { return Err(UnexpectedLength) }
                
                let mut capabilities = vec![];
                if data.len() > 7 {
                    for i in 7..data.len() {
                        capabilities.push(Capability::from(data[i]));
                    }
                }
                
                Ok(InfoResponse(ServerInfo {
                    header: [data[1], data[2], data[3], data[4]],
                    version: u16::from_be_bytes([data[5], data[6]]),
                    capabilities,
                }))
            },
            ID_QUEUE_REQ => Ok(QueueRequest),
            ID_QUEUE_RES => {
                if data.len() != 5 { return Err(UnexpectedLength) }
                
                Ok(QueueResponse(u32::from_be_bytes([data[1], data[2], data[3], data[4]])))
            },
            ID_IMAGE_REQ => Ok(ImageRequest),
            ID_IMAGE_RES => Ok(ImageResponse(data[1..].to_vec())),
            
            ID_REQ_DENIED => Ok(RequestDenied),
            _ => Ok(Unknown(data.to_vec()))
        }
    }
    
    pub fn id(&self) -> u8 {
        match self {
            Ping => ID_PING,
            Pong => ID_PONG,
            InfoRequest => ID_INFO_REQ,
            InfoResponse(_) => ID_INFO_RES,
            QueueRequest => ID_QUEUE_REQ,
            QueueResponse(_) => ID_QUEUE_RES,
            ImageRequest => ID_IMAGE_REQ,
            ImageResponse(_) => ID_IMAGE_RES,
            
            RequestDenied => ID_REQ_DENIED,
            Unknown(_) => ID_UNKNOWN
        }
    }
    
    pub fn serialize(&self) -> Vec<u8> {
        let mut raw = vec![self.id()];
        match self {
            Ping => (),
            Pong => (),
            InfoRequest => (),
            InfoResponse(info) => raw.extend_from_slice(&info.serialize()),
            QueueRequest => (),
            QueueResponse(data) => raw.extend_from_slice(&data.to_be_bytes()),
            ImageRequest => (),
            ImageResponse(data) => raw.extend_from_slice(&data),
            
            RequestDenied => (),
            Unknown(data) => raw.extend_from_slice(&data),
        }
        
        raw
    }
}
