
use std::time::Duration;
use remote64_common::{Feature, Packet};
use remote64_common::intercom::{Endpoint, InterMessage};
use remote64_common::network::Client;



pub struct SocketManager {
    pub socket: Client,
}
impl SocketManager {
    pub fn init(domain: Option<&str>, _features: Vec<Feature>, endpoint: Endpoint) {
        //let socket = websocket::ClientBuilder::new(&format!("ws://{}:6400", domain.unwrap_or("bigbass1997.com"))).unwrap().connect_insecure().unwrap();
        //socket.set_nonblocking(true).unwrap();
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
                                //socket.send_message(&OwnedMessage::Binary(Packet::FrameRequest.serialize())).unwrap();
                                endpoint.send.try_send(InterMessage::BulkFrames(frames)).unwrap_or_default();
                                
                                /*let elapsed = last_frame.elapsed();
                                info!("Last frame received: {:.3}ms | Download FPS: {:.2} | Size: {:.2}KiB vs Compress: {:.2}KiB",
                                    elapsed.as_micros() as f64 / 1000.0,
                                    1.0 / elapsed.as_secs_f64(),
                                    frame.video.len() as f64 / 1024.0,
                                    u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as f64 / 1024.0
                                );
                                last_frame = Instant::now();
                                
                                let elapsed = last_audio.elapsed();
                                info!("Last audio chunk:   {:.3}ms | Chunk Rate: {:.2}KHz | Size: {:.2}KiB",
                                    elapsed.as_micros() as f64 / 1000.0,
                                    frame.audio.len() as f64 / elapsed.as_secs_f64(),
                                    frame.audio.len() as f64 / 1024.0
                                );
                                last_audio = Instant::now();
                                for sample in frame.audio {
                                    audio_queue.push(sample);
                                }*/
                            },
                            Packet::RequestDenied => {
                                //socket.send_message(&OwnedMessage::Binary(Packet::FrameRequest.serialize())).unwrap();
                            }
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
            //sm.socket.send_message(&OwnedMessage::Close(None)).unwrap();
        }).unwrap();
    }
}