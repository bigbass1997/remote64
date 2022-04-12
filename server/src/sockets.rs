
use std::collections::vec_deque::VecDeque;
use std::time::{Duration, Instant};
use crossbeam_queue::SegQueue;
use remote64_common::{Feature, Packet, Packet::*, ServerInfo};
use remote64_common::intercom::{Endpoint, InterMessage};
use remote64_common::network::{Server, SocketConnection};

pub const INFO_HEADER: [u8; 4] = [0x52, 0x4D, 0x36, 0x34]; // RM64
pub const INFO_VERSION: u16 = 0x0000;


/// Contains the status of a connected client.
pub struct SocketClient {
    socket: SocketConnection,
    last_ping: Instant,
    last_pong: Instant,
    waiting: bool,
}
impl SocketClient {
    pub fn new(socket: SocketConnection) -> Self { Self {
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
    socket: Server,
    client_queue: VecDeque<SocketClient>,
}

impl SocketManager {
    pub fn init(features: Vec<Feature>, endpoint: Endpoint) {
        let server_info = ServerInfo {
            header: INFO_HEADER,
            version: INFO_VERSION,
            features,
        };
        
        let socket = Server::new("0.0.0.0:6400");
        
        let mut sm = SocketManager {
            socket,
            client_queue: VecDeque::new(),
        };
        
        let frame_queue = SegQueue::new();
        std::thread::Builder::new().name("SocketManager".to_owned()).spawn(move || {
            loop {
                // Accept any waiting connection requests, and add them to the queue
                while let Some(client) = sm.socket.accept() {
                    info!("Client {} connected.", client.peer);
                    sm.client_queue.push_back(SocketClient::new(client));
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
                        info!("Client {} is being serviced now.", client.socket.peer);
                    }
                    
                    while let Ok(msg) = client.socket.recv.try_recv() {
                            /*OwnedMessage::Close(_) => {
                                disconnects.push(i);
                                info!("Client {} disconnected.", client.socket.peer);
                                continue;
                            },*/
                        let packet = match Packet::deserialize(&msg) {
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
                                debug!("Ping! {}", client.socket.peer);
                                send_packet(client, Pong);
                            },
                            Pong => {
                                debug!("Pong! {}", client.socket.peer);
                                client.last_pong = Instant::now();
                            },
                            
                            FrameRequest(requested) if !client.waiting => { // if client is at front of queue
                                //debug!("Sending pong instead of frames.");
                                //send_packet(client, Pong);
                                //debug!("Sending blank frame.");
                                //send_packet(client, FrameResponse(vec![Frame::new(vec![231u8; 720*480*3], vec![])]));
                                let requested = requested as usize;
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
                                send_packet(client, packet);
                            }
                            FrameRequest(_) => send_packet(client, RequestDenied),
                            
                            InfoResponse(_) | QueueResponse(_) | FrameResponse(_) | RequestDenied | Unknown(_) => (),
                        }
                    }
                    if client.last_pong.elapsed() > Duration::from_secs(22) {
                        disconnects.push(i);
                        continue;
                    } else {
                        if client.last_ping.elapsed() > Duration::from_secs(10) {
                            debug!("Ping! {}", client.socket.peer);
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
                            while frame_queue.len() > 60 {
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
    client.socket.send.try_send(packet.serialize()).unwrap_or_default();
}