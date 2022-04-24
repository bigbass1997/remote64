
use std::time::Duration;
use remote64_common::{Feature, Packet};
use remote64_common::intercom::{Endpoint, InterMessage};
use remote64_common::network::Client;



pub struct SocketManager {
    pub socket: Client,
}
impl SocketManager {
    pub fn init(domain: Option<&str>, _features: Vec<Feature>, endpoint: Endpoint) {
        let socket = Client::new(&format!("{}:6400", domain.unwrap_or("bigbass1997.com")));
        
        let sm = SocketManager {
            socket,
        };
        
        std::thread::Builder::new().name("SocketManager".to_owned()).spawn(move || {
            'running: loop {
                // Handle any inbound messages from server
                while let Ok(msg) = sm.socket.recv.try_recv() {
                    match Packet::deserialize(&msg) {
                        Ok(packet) => match packet {
                            Packet::Ping => {
                                debug!("Ping! {}", sm.socket.peer);
                                sm.socket.send.try_send(Packet::Pong.serialize()).unwrap();
                            },
                            Packet::FrameResponse(frames) => {
                                endpoint.send.try_send(InterMessage::BulkFrames(frames)).unwrap_or_default();
                            },
                            _ => ()
                        },
                        _ => ()
                    }
                }
                
                loop {
                    match endpoint.recv.try_recv() {
                        Ok(msg) => {
                            match msg {
                                InterMessage::SocketPacket(packet) => {
                                    sm.socket.send.try_send(packet.serialize()).unwrap();
                                }
                                InterMessage::Kill => {
                                    break 'running;
                                },
                                _ => ()
                            }
                        },
                        Err(_) => break
                    }
                }
                
                std::thread::sleep(Duration::from_nanos(1));
            }
            
            sm.socket.send.send_timeout(Packet::Close.serialize(), Duration::from_secs(1)).unwrap_or_default();
        }).unwrap();
    }
}