use std::time::Duration;
use crossbeam_channel::{bounded, Receiver, Sender, unbounded};

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
    pub fn start(mut self) {
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
                                    self.channels[j].send.send(msg.clone()).unwrap_or_default()
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
                self.channels.remove(indices[i]);
            }
            indices.clear();
            
            std::thread::sleep(Duration::from_nanos(1));
        }
    }
}




#[derive(Clone, Debug)]
pub enum Event {
    Foo,
    Bar,
    Deadbeef(String),
    Kill,
}
use Event::*;

pub fn test() {
    let mut network: BroadcastNetwork<Event> = BroadcastNetwork::new();
    
    let channel = network.endpoint();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(2));
        
        channel.send.send(Foo).unwrap();
        
        loop {
            if let Ok(event) = channel.recv.recv_timeout(Duration::from_millis(100)) {
                println!("FirstThread: {:?}", event);
                
                match event {
                    Kill => break,
                    _ => ()
                }
            }
        }
        
        println!("FirstThread End");
    });
    
    let channel = network.endpoint();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(1));
        
        channel.send.send(Deadbeef("Fish".to_owned())).unwrap();
        
        loop {
            if let Ok(event) = channel.recv.recv_timeout(Duration::from_millis(100)) {
                println!("SecondThread: {:?}", event);
                
                match event {
                    Kill => break,
                    Bar => {
                        channel.send.send(Kill).unwrap();
                        break
                    },
                    _ => ()
                }
            }
        }
        
        println!("SecondThread End");
    });
    
    let channel = network.endpoint();
    std::thread::spawn(move || {
        loop {
            if let Ok(event) = channel.recv.recv_timeout(Duration::from_millis(1)) {
                println!("ThirdThread: {:?}", event);
                
                match event {
                    Foo => channel.send.send(Bar).unwrap(),
                    Kill => break,
                    _ => ()
                }
            }
        }
        
        println!("ThirdThread End");
    });
    
    
    println!("started");
    
    network.start();
    
    println!("all threads dead");
}