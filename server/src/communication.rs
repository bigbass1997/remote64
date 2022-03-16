use std::collections::VecDeque;
use std::net::TcpStream;
use std::time::{Duration, Instant};
use crossbeam_channel::{bounded, Receiver, Sender, unbounded};
use websocket::OwnedMessage;
use websocket::server::NoTlsAcceptor;
use websocket::sync::{Client, Server};
use remote64_common::{Capability, Packet, Packet::*, ServerInfo};

pub const INFO_HEADER: [u8; 4] = [0x52, 0x4D, 0x36, 0x34]; // RM64
pub const INFO_VERSION: u16 = 0x0000;

type WsClient = Client<TcpStream>;
type WsServer = Server<NoTlsAcceptor>;



/// A channel for sending and receiving messages with another TwoWayChannel.
/// 
/// Say you have two channels: A and B.
/// The type of data flowing from A to B, can be different than the type flowing from B to A, using generics.
#[derive(Clone, Debug)]
pub struct TwoWayChannel<A, B> {
    pub send: Sender<A>,
    pub recv: Receiver<B>,
}
impl<A, B> TwoWayChannel<A, B> {
    /// Creates both endpoints of the channel.
    /// 
    /// The sender of one endpoint, will send messages to the receiver of the other.
    pub fn new(bound: Option<usize>) -> (TwoWayChannel<A, B>, TwoWayChannel<B, A>) {
        let chan1 = match bound {
            Some(bound) => bounded(bound),
            None => unbounded()
        };
        let chan2 = match bound {
            Some(bound) => bounded(bound),
            None => unbounded()
        };
        
        (
            TwoWayChannel {
                send: chan1.0,
                recv: chan2.1,
            },
            TwoWayChannel {
                send: chan2.0,
                recv: chan1.1,
            }
        )
    }
}



/// Contains the status of a connected client.
pub struct SocketClient {
    socket: WsClient,
    last_ping: Instant,
    last_pong: Instant,
    requesting_image: bool,
}
impl SocketClient {
    pub fn new(socket: WsClient) -> Self { Self {
        socket: socket,
        last_ping: Instant::now(),
        last_pong: Instant::now(),
        requesting_image: false,
    }}
}


/// Handles websocket client connections.
/// 
/// Using threads, the socket manager will track all connected clients, keeping them
/// updated on their position in the queue, and removing any clients that disconnect or
/// fail to respond to pings.
/// 
/// The manager also acts as a relay between the active client and the rest of the
/// server's components (e.g. handling `Packet` transmissions).
pub struct SocketManager {
    socket: WsServer,
    client_queue: VecDeque<SocketClient>,
    last_frame: Vec<u8>,
    last_frame_stale: bool,
}

impl SocketManager {
    pub fn new(capabilities: Vec<Capability>) -> TwoWayChannel<Packet, Packet> {
        let server_info = ServerInfo {
            header: INFO_HEADER,
            version: INFO_VERSION,
            capabilities
        };
        
        let (chan_owned, chan_other) = TwoWayChannel::<Packet, Packet>::new(None);
        
        let socket = WsServer::bind("0.0.0.0:6400").unwrap();
        socket.set_nonblocking(true).unwrap();
        
        let mut sm = SocketManager {
            socket,
            client_queue: VecDeque::new(),
            last_frame: Default::default(),
            last_frame_stale: true,
        };
        
        std::thread::Builder::new().name("SocketManager".to_owned()).spawn(move || {
            loop {
                // Accept any waiting connection requests, and add them to the queue
                while let Ok(request) = sm.socket.accept() {
                    match request.accept() {
                        Ok(client) => {
                            match client.set_nonblocking(true) {
                                Ok(_) => (),
                                Err(err) => warn!("Failed to set client nonblocking: {:?}", err)
                            }
                            info!("Client {} connected.", parse_peer(&client));
                            sm.client_queue.push_back(SocketClient::new(client));
                            
                        },
                        Err(err) => warn!("Failed to accept client: {:?}", err)
                    }
                }
                
                // Client keep-alive and message passing
                //   Clients that have not responded to any pings for a period of time, will be
                //     disconnected and removed from the queue.
                //   This section also handles passing of messages between the server and clients.
                //   The client at the front of the queue is the "active" client. Image requests
                //     from "non-active" clients will be rejected.
                let mut disconnects = vec![];
                for (i, client) in sm.client_queue.iter_mut().enumerate() {
                    while let Ok(msg) = client.socket.recv_message() {
                        match msg {
                            OwnedMessage::Close(_) => {
                                disconnects.push(i);
                                info!("Client {} disconnected.", parse_peer(&client.socket));
                                continue;
                            },
                            OwnedMessage::Binary(data) => {
                                let packet = match Packet::deserialize(&data) {
                                    Ok(packet) => packet,
                                    Err(err) => {
                                        warn!("Malformed packet: {:?}", err);
                                        continue;
                                    }
                                };
                                
                                match packet {
                                    InfoRequest => send_packet(client, InfoResponse(server_info.clone())),
                                    QueueRequest => send_packet(client, QueueResponse(i as u32)),
                                    Ping => {
                                        debug!("Ping! {}", parse_peer(&client.socket));
                                        send_packet(client, Pong)
                                    },
                                    Pong => {
                                        debug!("Pong! {}", parse_peer(&client.socket));
                                        client.last_pong = Instant::now()
                                    },
                                    
                                    ImageRequest if i == 0 => { // if client is at front of queue
                                        client.requesting_image = true;
                                        chan_owned.send.try_send(packet).unwrap_or_default()
                                    }
                                    ImageRequest => send_packet(client, RequestDenied),
                                    
                                    InfoResponse(_) | QueueResponse(_) | ImageResponse(_) | AudioSamples(_) | RequestDenied | Unknown(_) => (),
                                }
                            },
                            _ => ()
                        }
                    }
                    if client.last_pong.elapsed() > Duration::from_secs(22) {
                        disconnects.push(i);
                        continue;
                    } else {
                        if client.last_ping.elapsed() > Duration::from_secs(10) {
                            debug!("Ping! {}", parse_peer(&client.socket));
                            send_packet(client, Ping);
                            client.last_ping = Instant::now();
                        }
                        
                        
                        if client.requesting_image && !sm.last_frame_stale {
                            match zstd::encode_all(sm.last_frame.as_slice(), 3) {
                                Ok(data) => {
                                    send_packet(client, ImageResponse(data));
                                },
                                Err(err) => warn!("Failed to compress image data: {:?}", err)
                            }
                            client.requesting_image = false;
                            sm.last_frame_stale = true;
                        }
                    }
                }
                disconnects.sort();
                for i in disconnects.iter().rev() {
                    sm.client_queue.remove(*i);
                }
                
                // Process any internally created packets, provided to the socket manager
                while let Ok(packet) = chan_owned.recv.try_recv() {
                    match packet {
                        ImageResponse(img) => { // Update internal state with latest image
                            sm.last_frame = img;
                            sm.last_frame_stale = false;
                        },
                        AudioSamples(samples) => {
                            if let Some(socket) = sm.client_queue.front_mut() {
                                debug!("Send_packet {} samples.", samples.len());
                                send_packet(socket, AudioSamples(samples));
                            }
                        }
                        _ => ()
                    }
                }
            }
        }).unwrap();
        
        chan_other
    }
}

fn send_packet(client: &mut SocketClient, packet: Packet) {
    client.socket.set_nonblocking(false).unwrap();
    match client.socket.send_message(&OwnedMessage::Binary(packet.serialize())) {
        Ok(_) => (),
        Err(err) => warn!("Failed to send message: {:?}", err)
    }
    client.socket.set_nonblocking(true).unwrap();
}

fn parse_peer(socket: &WsClient) -> String {
    match socket.peer_addr() {
        Ok(addr) => addr.to_string(),
        Err(_) => "unknown".to_owned()
    }
}