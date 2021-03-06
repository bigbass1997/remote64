use std::time::Duration;
use crossbeam_channel::{bounded, Receiver, Sender, unbounded};
use crate::{Frame, Packet};

/// A channel for sending and receiving messages with another BidirectionalChannel.
/// 
/// Say you have two channels: A and B.
/// The type of data flowing from A to B, can be different than the type flowing from B to A, using generics.
#[derive(Clone, Debug)]
pub struct BidirectionalChannel<A, B> {
    pub send: Sender<A>,
    pub recv: Receiver<B>,
}
impl<A, B> BidirectionalChannel<A, B> {
    /// Creates both endpoints of the channel.
    /// 
    /// The sender of one endpoint, will send messages to the receiver of the other.
    pub fn new(bound: Option<usize>) -> (BidirectionalChannel<A, B>, BidirectionalChannel<B, A>) {
        let chan1 = match bound {
            Some(bound) => bounded(bound),
            None => unbounded()
        };
        let chan2 = match bound {
            Some(bound) => bounded(bound),
            None => unbounded()
        };
        
        (
            BidirectionalChannel {
                send: chan1.0,
                recv: chan2.1,
            },
            BidirectionalChannel {
                send: chan2.0,
                recv: chan1.1,
            }
        )
    }
    
    pub fn split(self) -> (Sender<A>, Receiver<B>) {
        ( self.send, self.recv )
    }
}


/// A network of threadsafe, lockless, bidirectional channels.
/// 
/// Every endpoint can broadcast a message to all other endpoints at any time. Each endpoint is responsible
/// for processing all messages recieved from other endpoints.
/// 
/// All network endpoints must be created before messages can start being distributed. When ready,
/// the network can be started by calling `.start()`. This will take ownership of the network and
/// spawn a manager thread to handle passing broadcasts to every endpoint.
pub struct BroadcastNetwork<T: Clone> {
    channels: Vec<BidirectionalChannel<T, T>>,
}
impl<T: Clone> BroadcastNetwork<T> {
    pub fn new() -> Self {
        Self {
            channels: vec![],
        }
    }
    
    /// Creates a new endpoint on this network.
    /// 
    /// The endpoint can be used to send and recieve messages to/from all other endpoints; however it
    /// will not recieve its own sent messages.
    pub fn endpoint(&mut self) -> BidirectionalChannel<T, T> {
        let (chan_other, chan_owned) = BidirectionalChannel::<T, T>::new(None);
        
        self.channels.push(chan_owned);
        
        chan_other
    }
    
    /// Begins processing messages being sent in the network.
    /// 
    /// Notice! This call blocks until the network has no remaining endpoints.
    pub fn start(self) {
        let mut indices = Vec::with_capacity(self.channels.len());
        loop {
            if self.channels.is_empty() {
                break;
            }
            
            for i in 0..self.channels.len() {
                loop {
                    match self.channels[i].recv.try_recv() {
                        Ok(msg) => {
                            for j in 0..self.channels.len() {
                                if i != j {
                                    //println!("network sending message");
                                    self.channels[j].send.try_send(msg.clone()).unwrap_or_default();
                                    //println!("sent");
                                }
                            }
                        },
                        Err(err) => match err {
                            crossbeam_channel::TryRecvError::Disconnected => {
                                indices.push(i);
                                break
                            },
                            _ => break
                        }
                    }
                }
            }
            
            for i in (0..indices.len()).rev() {
                //self.channels.remove(indices[i]);
            }
            indices.clear();
            
            std::thread::sleep(Duration::from_nanos(1));
        }
    }
}


#[derive(Clone, Debug)]
pub enum InterMessage {
    SocketPacket(Packet),
    LatestFrame(Frame),
    BulkFrames(Vec<Frame>),
    StartRecording,
    StopRecording,
    
    Kill,
}

pub type Endpoint = BidirectionalChannel<InterMessage, InterMessage>;