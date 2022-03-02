
extern crate pretty_env_logger;
#[macro_use] extern crate log;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use crossbeam_channel::{bounded, unbounded};
use hound::WavWriter;
use image::{ImageFormat, RgbImage};
use minifb::{Key, Window, WindowOptions};
use minifb::{KeyRepeat, Scale, ScaleMode};
use portaudio::DeviceIndex;
use v4l::{Control, Device, FourCC};
use v4l::buffer::Type;
use v4l::format::{Colorspace, FieldOrder};
use v4l::io::mmap::Stream;
use v4l::io::traits::OutputStream;
use v4l::video::Capture;
use websocket::OwnedMessage;
use websocket::server::{NoTlsAcceptor};
use websocket::sync::Server as WsServer;
use remote64_common::Packet;
use remote64_common::util::InfCell;


mod communication;


const WIDTH: usize = 720;
const HEIGHT: usize = 480;

pub struct Server {
    pub socket: WsServer<NoTlsAcceptor>,
    running: bool,
    pub has_client: bool,
}

fn main() {
    pretty_env_logger::init();
    
    let serv = Arc::new(InfCell::new(Server {
        running: true,
        socket: WsServer::bind("0.0.0.0:6400").unwrap(),
        has_client: false,
    }));
    serv.get_mut().socket.set_nonblocking(true).unwrap();
    
    let (img_send, img_recv) = bounded(1);
    
    let serv_arc = serv.clone();
    std::thread::spawn(move || {
        let serv_copy = serv_arc.get_mut();
        while serv_copy.running {
            loop {
                let next = serv_copy.socket.next();
                if next.is_none() { break; }
                
                let next = next.unwrap();
                if next.is_ok() {
                    let request = next.unwrap();
                    
                    let img_recv = img_recv.clone();
                    let serv_copy = serv_arc.get_mut();
                    serv_copy.has_client = true;
                    std::thread::spawn(move || {
                        let client = request.accept().unwrap();
                        println!("New connection from: {}", client.peer_addr().unwrap());
                        let (mut receiver, mut sender) = client.split().unwrap();
                        
                        let mut client_ready = false;
                        loop {
                            match receiver.recv_message() {
                                Ok(msg) => match msg {
                                    OwnedMessage::Binary(data) => {
                                        match Packet::deserialize(&data).unwrap() {
                                            Packet::ImageRequest => {
                                                client_ready = true;
                                            }
                                            _ => ()
                                        }
                                    }
                                    _ => ()
                                },
                                Err(_) => ()
                            }
                            
                            match img_recv.recv() {
                                Ok(img_buf) => {
                                    if client_ready {
                                        match sender.send_message(&OwnedMessage::Binary(img_buf)) {
                                            Ok(_) => (),
                                            Err(err) => {
                                                println!("Socket Error: {:?}", err);
                                                serv_copy.has_client = false;
                                                break;
                                            }
                                        }
                                        client_ready = false;
                                    }
                                },
                                Err(err) => println!("Channel Error: {:?}", err)
                            }
                        }
                    });
                }
            }
            
            if !serv_copy.has_client {
                while let Ok(_) = img_recv.try_recv() {}
            }
            
            std::thread::sleep(Duration::from_secs_f32(0.5));
        }
    });
    
    
    let mut window_buf: Vec<u32> = vec![0; WIDTH * HEIGHT];
    let mut window = Window::new("remote64-server", WIDTH, HEIGHT, WindowOptions {
        borderless: false,
        title: false,
        resize: false,
        scale: Scale::X1,
        scale_mode: ScaleMode::AspectRatioStretch,
        topmost: false,
        transparency: false,
        none: false
    }).unwrap();
    
    window.limit_update_rate(Some(Duration::from_secs_f32(1.0/60.0)));
    
    let mut dev = Device::new(0).unwrap();
    let mut sharp_val = 0;
    let mut sharp_control_id = 0;
    for control in dev.query_controls().unwrap() {
        if control.name == "Sharpness" {
            sharp_control_id = control.id;
            sharp_val = match dev.control(control.id).unwrap() {
                Control::Value(val) => val,
                _ => 0,
            };
        }
        
        if control.name == "Mute" {
            dev.set_control(control.id, Control::Value(0)).unwrap();
        }
    }
    
    let mut fmt = dev.format().unwrap();
    fmt.width = WIDTH as u32;
    fmt.height = HEIGHT as u32;
    fmt.fourcc = FourCC::new(b"RGBP");
    fmt.colorspace = Colorspace::NTSC;
    fmt.field_order = FieldOrder::Alternate;
    dev.set_format(&fmt).unwrap();
    
    
    
    let pa = portaudio::PortAudio::new().unwrap();
    
    let mut input_device_id = DeviceIndex(0);
    for device in pa.devices().unwrap() {
        let (idx, info) = device.unwrap();
        
        if info.name.contains("pulse") {
            input_device_id = idx;
        }
    }
    //input_device_id = pa.default_input_device().unwrap();
    let input_device_info = pa.device_info(input_device_id).unwrap();
    let latency = input_device_info.default_low_input_latency;
    let input_params = portaudio::StreamParameters::<f32>::new(input_device_id, 2, true, latency);
    
    
    let output_device_id = pa.default_output_device().unwrap();
    let output_device_info = pa.device_info(output_device_id).unwrap();
    let latency = output_device_info.default_low_output_latency;
    let output_params = portaudio::StreamParameters::<f32>::new(output_device_id, 2, true, latency);
    
    pa.is_duplex_format_supported(input_params, output_params, 44100.0).unwrap();
    
    let settings = portaudio::DuplexStreamSettings::new(input_params, output_params, 44100.0, 512);
    
    let mut wav_writer = get_wav_writer("recorded.wav", 2, 44100.0).unwrap();
    
    
    let callback = move |portaudio::stream::DuplexCallbackArgs {
                             in_buffer,
                             out_buffer,
                             frames: _,
                             flags, 
                             time: _,
                         }| {
        if !flags.is_empty() {
            println!("flags: {:?}", flags);
        }
        
        for (output_sample, input_sample) in out_buffer.iter_mut().zip(in_buffer.iter()) {
            *output_sample = *input_sample;
            wav_writer.write_sample(*input_sample).unwrap();
        }
        
        portaudio::Continue
    };
    
    let mut audio_stream = pa.open_non_blocking_stream(settings, callback).unwrap();
    audio_stream.start().unwrap();
    
    
    
    let mut img = RgbImage::new(WIDTH as u32, HEIGHT as u32);
    
    let mut stream = Stream::with_buffers(&mut dev, Type::VideoCapture, 4).unwrap();
    stream.next().unwrap(); // first frame is always black?
    stream.next().unwrap(); // first frame is always black?
    
    window_buf.fill(0);
    let mut frame_num = 0usize;
    let mut socket_buf = vec![0; window_buf.len() * 3];
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let (stream_buf, _meta) = stream.next().unwrap(); // blocks until next frame, thus may limit FPS
        
        if window.is_key_pressed(Key::Right, KeyRepeat::No) {
            sharp_val += 1;
            if sharp_val > 15 { sharp_val = 15; }
            dev.set_control(sharp_control_id, Control::Value(sharp_val)).unwrap();
            println!("Set sharpness: {}", sharp_val);
        }
        if window.is_key_pressed(Key::Left, KeyRepeat::No) {
            sharp_val -= 1;
            if sharp_val < 0 { sharp_val = 0; }
            dev.set_control(sharp_control_id, Control::Value(sharp_val)).unwrap();
            println!("Set sharpness: {}", sharp_val);
        }
        
        for i in (0..stream_buf.len()).step_by(2) {
            let r = ((stream_buf[i + 1] & 0b11111000) >> 3) * 8;
            let g = (((stream_buf[i + 1] & 0b00000111) << 3) | ((stream_buf[i] & 0b11100000) >> 5)) * 4;
            let b = (stream_buf[i] & 0b00011111) << 3;
            let color = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
            
            let i = i / 2;
            
            window_buf[i] = color;
            
            let x = i % WIDTH;
            let y = i / WIDTH;
            (*img.get_pixel_mut(x as u32, y as u32)).0 = [r, g, b];
        }
        
        window.update_with_buffer(&window_buf, WIDTH, HEIGHT).unwrap();
        //img.save(format!("video/output-{:08}.bmp", frame_num)).unwrap();
        for i in 0..window_buf.len() {
            let color = window_buf[i];
            socket_buf[(i * 3) + 0] = ((color & 0xFF0000) >> 16) as u8;
            socket_buf[(i * 3) + 1] = ((color & 0x00FF00) >> 8) as u8;
            socket_buf[(i * 3) + 2] = (color & 0x0000FF) as u8;
        }
        match img_send.try_send(zstd::encode_all(&*socket_buf, 3).unwrap()) {
            Ok(_) | Err(_) => ()
        }
        //img_send.send(socket_buf.clone()).unwrap();
        frame_num += 1;
    }
    
    audio_stream.stop().unwrap();
}


fn get_wav_writer(path: &'static str, channels: i32, sample_rate: f64) -> Result<WavWriter<std::io::BufWriter<std::fs::File>>,String> {
    let spec = wav_spec(channels, sample_rate);
    match hound::WavWriter::create(path, spec) {
        Ok(writer) => Ok(writer),
        Err(error) => Err (String::from(format!("{}",error))),
    }
}

fn wav_spec(channels: i32, sample_rate: f64) -> hound::WavSpec {
    hound::WavSpec {
        channels: channels as _,
        sample_rate: sample_rate as _,
        bits_per_sample: (std::mem::size_of::<f32>() * 8) as _,
        sample_format: hound::SampleFormat::Float,
    }
}