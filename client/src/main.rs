use std::time::{Duration, Instant};
use minifb::{Key, Scale, ScaleMode, Window, WindowOptions};
use websocket::OwnedMessage;
use remote64_common::Packet;

const WIDTH: usize = 720;
const HEIGHT: usize = 480;

fn main() {
    let mut window_buf: Vec<u32> = vec![0; WIDTH * HEIGHT];
    let mut window = Window::new("remote64-client", WIDTH, HEIGHT, WindowOptions {
        borderless: false,
        title: false,
        resize: false,
        scale: Scale::X1,
        scale_mode: ScaleMode::AspectRatioStretch,
        topmost: false,
        transparency: false,
        none: false
    }).unwrap();
    
    window.limit_update_rate(Some(Duration::from_secs_f32(1.0/30.0)));
    
    println!("Attempting to connect...");
    let mut socket = websocket::ClientBuilder::new("ws://bigbass1997.com:6400").unwrap().connect_insecure().unwrap();
    println!("Connected!");
    
    std::thread::sleep(Duration::from_secs(2));
    
    socket.send_message(&OwnedMessage::Binary(Packet::ImageRequest.serialize())).unwrap();
    
    let mut last_frame = Instant::now();
    while window.is_open() && !window.is_key_down(Key::Escape) {
        match socket.recv_message() {
            Ok(msg) => match msg {
                OwnedMessage::Close(_) => {
                    info!("Connection closed by server.");
                    return;
                },
                OwnedMessage::Binary(data) => {
                    match Packet::deserialize(&data) {
                        Ok(packet) => match packet {
                            Packet::Ping => socket.send_message(&OwnedMessage::Binary(Packet::Pong.serialize())).unwrap(),
                            Packet::ImageResponse(data) => {
                                let compressed_len = data.len();
                                let data = zstd::decode_all(&*data).unwrap();
                                for i in 0..window_buf.len() {
                                    let r = data[(i * 3)];
                                    let g = data[(i * 3) + 1];
                                    let b = data[(i * 3) + 2];
                                    window_buf[i] = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                                    //println!("i: {:05} | {:08X}", i, window_buf[i]);
                                }
                                
                                let elapsed = last_frame.elapsed().as_secs_f64();
                                println!("Last frame received: {:.3}ms | Download FPS: {:.2} | Size: {:.2}KiB vs Compress: {:.2}KiB",
                                    last_frame.elapsed().as_micros() as f64 / 1000.0,
                                    1.0 / elapsed,
                                    data.len() as f64 / 1024.0,
                                    compressed_len as f64 / 1024.0
                                );
                                last_frame = Instant::now();
                                
                                socket.send_message(&OwnedMessage::Binary(Packet::ImageRequest.serialize())).unwrap();
                            },
                            _ => ()
                        },
                        Err(_) => ()
                    }
                }
                _ => ()
            },
            Err(_) => ()
        }
        
        window.update_with_buffer(&window_buf, WIDTH, HEIGHT).unwrap();
    }
    
    socket.send_message(&OwnedMessage::Close(None)).unwrap();
}