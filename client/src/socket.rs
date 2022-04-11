
use std::net::TcpStream;
use std::time::Duration;
use websocket::sync::Client;
use remote64_common::{Feature, Packet};
use remote64_common::intercom::{Endpoint, InterMessage};
use crate::OwnedMessage;


type WsClient = Client<TcpStream>;


pub struct SocketManager {
    pub socket: WsClient,
}
impl SocketManager {
    pub fn init(domain: Option<&str>, _features: Vec<Feature>, endpoint: Endpoint) {
        let socket = websocket::ClientBuilder::new(&format!("ws://{}:6400", domain.unwrap_or("bigbass1997.com"))).unwrap().connect_insecure().unwrap();
        socket.set_nonblocking(true).unwrap();
        
        let mut sm = SocketManager {
            socket,
        };
        
        std::thread::Builder::new().name("SocketManager".to_owned()).spawn(move || {
            'running: loop {
                sm.socket.recv_message();
                // Handle any inbound messages from server
                /*while let Ok(msg) = sm.socket.recv_message() {
                    println!("msg recv'ed");
                    match msg {
                        OwnedMessage::Close(_) => {
                            info!("Connection closed by server.");
                            break 'running;
                        },
                        OwnedMessage::Binary(data) => {
                            println!("binary len: {}", data.len());
                            match Packet::deserialize(&data) {
                                Ok(packet) => match packet {
                                    Packet::Ping => {
                                        debug!("Ping! {}", sm.socket.peer_addr().unwrap());
                                        sm.socket.send_message(&OwnedMessage::Binary(Packet::Pong.serialize())).unwrap()
                                    },
                                    Packet::FrameResponse(frames) => {
                                        //socket.send_message(&OwnedMessage::Binary(Packet::FrameRequest.serialize())).unwrap();
                                        println!("frameresponse");
                                        let p = InterMessage::BulkFrames(frames);
                                        //endpoint.send.try_send(InterMessage::BulkFrames(frames)).unwrap_or_default();
                                        println!("fr done");
                                        
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
                                Err(_) => ()
                            }
                        },
                        _ => ()
                    }
                    println!("msg matching finished");
                }*/
                
                loop {
                    match endpoint.recv.try_recv() {
                        Ok(msg) => {
                            println!("msg...");
                            match msg {
                                InterMessage::SocketPacket(packet) => {
                                    println!("endpoint send packet");
                                    send_packet(&mut sm.socket, packet);
                                    println!("endpoint send packet done");
                                }
                                InterMessage::Kill => {
                                    break 'running;
                                },
                                _ => ()
                            }
                        },
                        Err(err) => {
                            //println!("ERR: {}", err);
                            break;
                        }
                    }
                }
                
                std::thread::sleep(Duration::from_nanos(1));
            }
            
            sm.socket.send_message(&OwnedMessage::Close(None)).unwrap();
        }).unwrap();
    }
}

fn send_packet(socket: &mut WsClient, packet: Packet) {
    socket.set_nonblocking(false).unwrap();
    match socket.send_message(&OwnedMessage::Binary(packet.serialize())) {
        Ok(_) => (),
        Err(err) => warn!("Failed to send message: {:?}", err)
    }
    socket.set_nonblocking(true).unwrap();
}