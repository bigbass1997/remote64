
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
pub const ID_AUDIO_SAMPLE: u8 = 0x09;
pub const ID_REQ_DENIED: u8 = 0xFE;
pub const ID_UNKNOWN: u8 = 0xFF;


#[derive(Copy, Clone, Debug, PartialEq, FromPrimitive, IntoPrimitive, EnumString)]
#[repr(u8)]
pub enum Feature {
    LivePlayback = 0x01,
    AudioRecording = 0x02,
    InputHandling = 0x03,
    
    #[num_enum(default)]
    Invalid = 0x00,
}
impl Default for Feature {
    fn default() -> Self {
        Feature::Invalid
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct ServerInfo {
    pub header: [u8; 4],
    pub version: u16,
    pub features: Vec<Feature>,
}
impl ServerInfo {
    pub fn serialize(&self) -> Vec<u8> {
        let mut raw = vec![];
        raw.copy_from_slice(&self.header);
        raw.copy_from_slice(&self.version.to_be_bytes());
        for feat in &self.features {
            raw.push((*feat).into());
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
    ImageRequest, //TODO: Add image formatting and options to request (allow client to specify lower resolutions or lossy quality)
    ImageResponse(Vec<u8>), //TODO: Add image datastructure to convey format of image (necessary once resolution/lossy options are implemented)
    AudioSamples(Vec<f32>),
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
                
                let mut features = vec![];
                if data.len() > 7 {
                    for i in 7..data.len() {
                        features.push(Feature::from(data[i]));
                    }
                }
                
                Ok(InfoResponse(ServerInfo {
                    header: [data[1], data[2], data[3], data[4]],
                    version: u16::from_be_bytes([data[5], data[6]]),
                    features,
                }))
            },
            ID_QUEUE_REQ => Ok(QueueRequest),
            ID_QUEUE_RES => {
                if data.len() != 5 { return Err(UnexpectedLength) }
                
                Ok(QueueResponse(u32::from_be_bytes([data[1], data[2], data[3], data[4]])))
            },
            ID_IMAGE_REQ => Ok(ImageRequest),
            ID_IMAGE_RES => Ok(ImageResponse(data[1..].to_vec())),
            ID_AUDIO_SAMPLE => {
                if (data.len() - 1) % 4 != 0 { return Err(UnexpectedLength) }
                
                let mut samples = vec![];
                for i in (1..data.len()).step_by(4) {
                    samples.push(f32::from_be_bytes([data[i + 0], data[i + 1], data[i + 2], data[i + 3]]));
                }
                
                Ok(AudioSamples(samples))
            },
            
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
            AudioSamples(_) => ID_AUDIO_SAMPLE,
            
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
            AudioSamples(data) => {
                for sample in data {
                    raw.extend_from_slice(&sample.to_be_bytes());
                }
            },
            
            RequestDenied => (),
            Unknown(data) => raw.extend_from_slice(&data),
        }
        
        raw
    }
}
