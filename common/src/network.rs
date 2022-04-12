use std::cmp::min;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Duration;
use crossbeam_channel::{Receiver, Sender};
use crossbeam_queue::SegQueue;
use log::{debug, trace};
use crate::intercom::BidirectionalChannel;

pub type Message = Vec<u8>;

pub struct Server {
    new_connections: Arc<SegQueue<SocketConnection>>,
}
impl Server {
    pub fn new<A: ToSocketAddrs>(addr: A) -> Self {
        let listener = TcpListener::bind(addr).unwrap();
        listener.set_nonblocking(false).unwrap();
        
        let new_connections = Arc::new(SegQueue::new());
        let connections = new_connections.clone();
        std::thread::spawn(move || {
            loop {
                while let Ok((stream, _)) = listener.accept() {
                    connections.push(SocketConnection::new(stream));
                }
            }
        });
        
        Self {
            new_connections,
        }
    }
    
    pub fn accept(&self) -> Option<SocketConnection> {
        self.new_connections.pop()
    }
}


pub struct Client(SocketConnection);
impl Client {
    pub fn new<A: ToSocketAddrs>(addr: A) -> Self {
        let stream = TcpStream::connect(addr).unwrap();
        let connection = SocketConnection::new(stream);
        
        Self(connection)
    }
}
impl Deref for Client {
    type Target = SocketConnection;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Client {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}




#[derive(Default, Debug)]
struct WorkingMessage {
    msg_len: u32,
    payload: Vec<u8>,
}

pub struct SocketConnection {
    pub send: Sender<Message>,
    pub recv: Receiver<Message>,
    pub peer: SocketAddr,
}
impl SocketConnection {
    pub fn new(mut stream: TcpStream) -> Self {
        let (chan_pub, chan_owned) = BidirectionalChannel::<Message, Message>::new(None);
        
        stream.set_nonblocking(true).unwrap();
        stream.set_nodelay(true).unwrap();
        let peer = stream.peer_addr().unwrap();
        
        let mut workmsg_opt: Option<WorkingMessage> = None;
        let (send, recv) = chan_owned.split();
        std::thread::spawn(move || {
            loop {
                while let Ok(msg) = recv.try_recv() {
                    let mut buf = vec![];
                    buf.extend_from_slice(&(msg.len() as u32).to_be_bytes());
                    buf.extend_from_slice(&msg);
                    
                    trace!("Sending message. msg_len: {}, buf_len: {}", msg.len(), buf.len());
                    let mut i = 0;
                    while i < buf.len() {
                        let send_len = min(buf.len() - i, 1024 * 1024);
                        
                        let len = match stream.write(&buf[i..(i + send_len)]) {
                            Ok(len) => len,
                            Err(_) => 0,
                        };
                        if len > 0 {
                            trace!("Wrote {} bytes", len);
                        }
                        i += len;
                    }
                    trace!("Message sent.");
                }
                
                if workmsg_opt.is_none() {
                    let mut msg = WorkingMessage::default();
                    let mut len_buf = [0u8; 4];
                    match stream.read_exact(&mut len_buf) {
                        Ok(_) => (),
                        Err(err) => {
                            trace!("Unable to read message len. {}", err);
                            std::thread::sleep(Duration::from_secs(1));
                            continue;
                        }
                    }
                    msg.msg_len = u32::from_be_bytes(len_buf);
                    trace!("Reading new message with len {} bytes", msg.msg_len);
                    
                    workmsg_opt = Some(msg);
                }
                
                let workmsg = workmsg_opt.as_mut().unwrap();
                
                let mut buf = vec![0u8; min(workmsg.msg_len as usize - workmsg.payload.len(), 1024 * 1024)];
                let len = match stream.read(&mut buf) {
                    Ok(len) => len,
                    Err(_) => 0
                };
                if len > 0 {
                    trace!("Read {} bytes", len);
                }
                
                if len > 0 {
                    workmsg.payload.extend_from_slice(&buf[0..len]);
                }
                
                if workmsg.payload.len() as u32 == workmsg.msg_len {
                    trace!("Finished reading new message. len: {}", workmsg.payload.len());
                    send.try_send(workmsg_opt.unwrap().payload).unwrap_or_default();
                    workmsg_opt = None;
                }
                
                //std::thread::sleep(Duration::from_nanos(1));
            }
        });
        
        let (send, recv) = chan_pub.split();
        Self {
            send,
            recv,
            peer,
        }
    }
}