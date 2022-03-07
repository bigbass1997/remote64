use std::collections::VecDeque;
use std::net::TcpStream;
use std::time::{Duration, Instant};
use crossbeam_channel::{bounded, Receiver, Sender};
use websocket::OwnedMessage;
use websocket::server::NoTlsAcceptor;
use websocket::sync::{Client, Server};
use remote64_common::{Capability, Packet, Packet::*, ServerInfo};

pub const INFO_HEADER: [u8; 4] = [0x52, 0x4D, 0x36, 0x34]; // RM64
pub const INFO_VERSION: u16 = 0x0000;

type WsClient = Client<TcpStream>;
type WsServer = Server<NoTlsAcceptor>;



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
    active_client: Option<SocketClient>,
}

impl SocketManager {
    pub fn new(capabilities: Vec<Capability>) -> (Receiver<()>, Sender<Vec<u8>>) {
        let server_info = ServerInfo {
            header: INFO_HEADER,
            version: INFO_VERSION,
            capabilities
        };
        
        let (request_image_send, request_image_recv) = bounded(1);
        let (image_response_send, image_response_recv) = bounded::<Vec<u8>>(1); // TODO: Change this to a channel of Packets instead of just images
        
        
        let socket = WsServer::bind("0.0.0.0:6400").unwrap();
        socket.set_nonblocking(true).unwrap();
        
        let mut sm = SocketManager {
            socket,
            client_queue: VecDeque::new(),
            active_client: None,
        };
        
        std::thread::Builder::new().name("SocketManager".to_owned()).spawn(move || {
            loop {
                while let Ok(request) = sm.socket.accept() {
                    match request.accept() {
                        Ok(client) => {
                            match client.set_nonblocking(true) {
                                Ok(_) => (),
                                Err(err) => warn!("Failed to set client nonblocking: {:?}", err)
                            }
                            if let Ok(peer) = client.peer_addr() {
                                info!("Client {} connected.", peer);
                            } else {
                                info!("Client connected.");
                            }
                            sm.client_queue.push_back(SocketClient::new(client));
                            
                        },
                        Err(err) => warn!("Failed to accept client: {:?}", err)
                    }
                }
                
                let mut disconnects = vec![];
                for (i, client) in sm.client_queue.iter_mut().enumerate() {
                    while let Ok(msg) = client.socket.recv_message() {
                        match msg {
                            OwnedMessage::Close(_) => {
                                disconnects.push(i);
                                if let Ok(peer) = client.socket.peer_addr() {
                                    info!("Client {} disconnected.", peer);
                                } else {
                                    info!("Client disconnected.");
                                }
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
                                    Ping => send_packet(client, Pong),
                                    Pong => client.last_pong = Instant::now(),
                                    
                                    ImageRequest => send_packet(client, RequestDenied),
                                    
                                    InfoResponse(_) | ImageResponse(_) | RequestDenied | Unknown(_) => (),
                                }
                            },
                            _ => ()
                        }
                    }
                    if client.last_pong.elapsed() > Duration::from_secs(45) {
                        disconnects.push(i);
                        continue;
                    }
                    if client.last_ping.elapsed() > Duration::from_secs(25) {
                        send_packet(client, Ping);
                        client.last_ping = Instant::now();
                    }
                }
                disconnects.sort();
                for i in disconnects.iter().rev() {
                    sm.client_queue.remove(*i);
                }
                
                
                if sm.active_client.is_none() && !sm.client_queue.is_empty() {
                    sm.active_client = sm.client_queue.pop_front();
                }
                
                let mut disconnect = false;
                if let Some(client) = &mut sm.active_client {
                    while let Ok(msg) = client.socket.recv_message() {
                        match msg {
                            OwnedMessage::Close(_) => {
                                disconnect = true;
                                break;
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
                                    Ping => send_packet(client, Pong),
                                    Pong => {
                                        client.last_pong = Instant::now();
                                        debug!("Pong from {}", client.socket.peer_addr().unwrap());
                                    },
                                    
                                    ImageRequest => {
                                        client.requesting_image = true;
                                        match request_image_send.try_send(()) { _ => () }
                                    },
                                    
                                    InfoResponse(_) | ImageResponse(_) | RequestDenied | Unknown(_) => (),
                                }
                            },
                            _ => ()
                        }
                    }
                    debug!("last_pong elapsed: {}sec", client.last_pong.elapsed().as_secs());
                    if client.last_pong.elapsed() > Duration::from_secs(15) {
                        disconnect = true;
                    } else {
                        if client.last_ping.elapsed() > Duration::from_secs(5) {
                            send_packet(client, Ping);
                            client.last_ping = Instant::now();
                        }
                        
                        if client.requesting_image {
                            if let Ok(image) = image_response_recv.try_recv() {
                                match zstd::encode_all(image.as_slice(), 3) {
                                    Ok(data) => {
                                        send_packet(client, ImageResponse(data));
                                    },
                                    Err(err) => warn!("Failed to compress image data: {:?}", err)
                                }
                                client.requesting_image = false;
                            }
                        }
                    }
                }
                if disconnect {
                    let client = sm.active_client.as_ref().unwrap();
                    if let Ok(peer) = client.socket.peer_addr() {
                        info!("Client {} disconnected.", peer);
                    } else {
                        info!("Client disconnected.");
                    }
                    client.socket.shutdown().unwrap_or_default();
                    sm.active_client = None;
                }
            }
        }).unwrap();
        
        (request_image_recv, image_response_send)
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