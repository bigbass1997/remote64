
use std::collections::vec_deque::VecDeque;
use std::net::TcpStream;
use std::time::{Duration, Instant};
use crossbeam_queue::SegQueue;
use websocket::OwnedMessage;
use websocket::server::NoTlsAcceptor;
use websocket::sync::{Client, Server};
use remote64_common::{Feature, Frame, Packet, Packet::*, ServerInfo};
use remote64_common::intercom::{Endpoint, InterMessage};

pub const INFO_HEADER: [u8; 4] = [0x52, 0x4D, 0x36, 0x34]; // RM64
pub const INFO_VERSION: u16 = 0x0000;

type WsClient = Client<TcpStream>;
type WsServer = Server<NoTlsAcceptor>;



/// Contains the status of a connected client.
pub struct SocketClient {
    socket: WsClient,
    last_ping: Instant,
    last_pong: Instant,
    waiting: bool,
}
impl SocketClient {
    pub fn new(socket: WsClient) -> Self { Self {
        socket: socket,
        last_ping: Instant::now(),
        last_pong: Instant::now(),
        waiting: true,
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
}

impl SocketManager {
    pub fn init(features: Vec<Feature>, endpoint: Endpoint) {
        let server_info = ServerInfo {
            header: INFO_HEADER,
            version: INFO_VERSION,
            features,
        };
        
        let socket = WsServer::bind("0.0.0.0:6400").unwrap();
        socket.set_nonblocking(true).unwrap();
        
        let mut sm = SocketManager {
            socket,
            client_queue: VecDeque::new(),
        };
        
        let frame_queue = SegQueue::new();
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
                            client.set_nonblocking(true).unwrap();
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
                    if i == 0 && client.waiting {
                        client.waiting = false;
                        endpoint.send.try_send(InterMessage::StartRecording).unwrap_or_default();
                        info!("Client {} is being serviced now.", parse_peer(&client.socket));
                    }
                    
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
                                        //send_packet(client, Pong);
                                        send_packet(client, FrameResponse(vec![Frame::new(vec![213u8; 720*50*3], vec![])]));
                                    },
                                    Pong => {
                                        debug!("Pong! {}", parse_peer(&client.socket));
                                        client.last_pong = Instant::now();
                                    },
                                    
                                    FrameRequest(requested) if !client.waiting => { // if client is at front of queue
                                        //debug!("Sending pong instead of frames.");
                                        //send_packet(client, Pong);
                                        debug!("Sending blank frame.");
                                        send_packet(client, FrameResponse(vec![Frame::new(vec![123u8; 720*480*3], vec![])]));
                                        /*let requested = requested as usize;
                                        let available = frame_queue.len();
                                        let to_send = if available <= requested {
                                            available
                                        } else {
                                            requested
                                        };
                                        
                                        let mut frames = vec![];
                                        for _ in 0..to_send {
                                            match frame_queue.pop() {
                                                Some(frame) => frames.push(frame),
                                                None => break
                                            }
                                        }
                                        let len = frames.len();
                                        let packet = FrameResponse(frames);
                                        let data = packet.serialize();
                                        
                                        debug!("Sending {} frames. Size: {:.2} KiB", len, data.len() as f64 / 1024.0);
                                        send_packet(client, packet);*/
                                    }
                                    FrameRequest(_) => send_packet(client, RequestDenied),
                                    
                                    InfoResponse(_) | QueueResponse(_) | FrameResponse(_) | RequestDenied | Unknown(_) => (),
                                }
                            },
                            _ => ()
                        }
                    }
                    if client.last_pong.elapsed() > Duration::from_secs(22) {
                        disconnects.push(i);
                        continue;
                    } else {
                        if client.last_ping.elapsed() > Duration::from_secs(3) {
                            debug!("Ping! {}", parse_peer(&client.socket));
                            send_packet(client, Ping);
                            client.last_ping = Instant::now();
                        }
                    }
                }
                disconnects.sort();
                for i in disconnects.iter().rev() {
                    if !sm.client_queue[*i].waiting {
                        endpoint.send.try_send(InterMessage::StopRecording).unwrap_or_default();
                    }
                    sm.client_queue.remove(*i);
                }
                
                // Process any internally created messages, provided to the socket manager
                while let Ok(msg) = endpoint.recv.try_recv() {
                    match msg {
                        InterMessage::LatestFrame(frame) => {
                            frame_queue.push(frame);
                            while frame_queue.len() > 30 {
                                frame_queue.pop();
                            }
                        },
                        _ => ()
                    }
                }
                
                std::thread::sleep(Duration::from_nanos(1));
            }
        }).unwrap();
    }
}

fn send_packet(client: &mut SocketClient, packet: Packet) {
    //client.socket.set_nonblocking(false).unwrap();
    match client.socket.send_message(&OwnedMessage::Binary(packet.serialize())) {
        Ok(_) => (),
        Err(err) => warn!("Failed to send message: {:?}", err)
    }
    //client.socket.set_nonblocking(true).unwrap();
}

fn parse_peer(socket: &WsClient) -> String {
    match socket.peer_addr() {
        Ok(addr) => addr.to_string(),
        Err(_) => "unknown".to_owned()
    }
}