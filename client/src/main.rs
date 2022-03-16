
extern crate env_logger;
#[macro_use] extern crate log;

use std::sync::Arc;
use std::time::{Duration, Instant};
use clap::{AppSettings, Arg, Command};
use crossbeam_queue::SegQueue;
use log::LevelFilter;
use minifb::{Key, Scale, ScaleMode, Window, WindowOptions};
use websocket::OwnedMessage;
use remote64_common::Packet;

const WIDTH: usize = 720;
const HEIGHT: usize = 480;

fn main() {
    // Run clap to parse cli arguments
    let matches = Command::new("remote64-server")
        .version(clap::crate_version!())
        .arg(Arg::new("log-level")
            .long("log-level")
            .takes_value(true)
            .default_value("info")
            .possible_values(["error", "warn", "info", "debug", "trace"])
            .help("Specify the console log level. Environment variable 'RUST_LOG' will override this option."))
        .arg(Arg::new("features")
            .short('f')
            .long("feature")
            .takes_value(true)
            .multiple_occurrences(true)
            .possible_values(["LivePlayback", "AudioRecording", "InputHandling"])
            .help("Specify a feature you wish to use if available. Use multiple -f/--feature args to specify multiple features."))
        .next_line_help(true)
        .setting(AppSettings::DeriveDisplayOrder)
        .get_matches();
    
    // Setup program-wide logger format
    let level = match std::env::var("RUST_LOG").unwrap_or(matches.value_of("log-level").unwrap_or("info").to_owned()).as_str() {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info
    };
    {
        let mut logbuilder = remote64_common::logger::builder();
        logbuilder.filter_level(level);
        logbuilder.init();
    }
    
    let pa = portaudio::PortAudio::new().unwrap();
    let output_device_id = pa.default_output_device().unwrap();
    let output_device_info = pa.device_info(output_device_id).unwrap();
    let latency = output_device_info.default_low_output_latency;
    let output_params = portaudio::StreamParameters::<f32>::new(output_device_id, 2, true, latency);
    
    
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
    
    info!("Attempting to connect...");
    let mut socket = websocket::ClientBuilder::new("ws://bigbass1997.com:6400").unwrap().connect_insecure().unwrap();
    info!("Connected!");
    
    std::thread::sleep(Duration::from_secs(1));
    
    
    pa.is_output_format_supported(output_params, 44100.0).unwrap();
    
    let settings = portaudio::OutputStreamSettings::new(output_params, 44100.0, 512);
    
    let sample_queue = Arc::new(SegQueue::new());
    let callback_sample_queue = sample_queue.clone();
    let callback = move |portaudio::stream::OutputCallbackArgs {
                             buffer,
                             frames: _,
                             flags, 
                             time: _,
                        }| {
        if !flags.is_empty() {
            debug!("flags: {:?}", flags);
        }
        
        for output_sample in buffer.iter_mut() {
            if let Some(sample) = callback_sample_queue.pop() {
                *output_sample = sample;
            } else {
                *output_sample = 0.0;
            }
        }
        
        portaudio::Continue
    };
    
    let mut audio_stream = pa.open_non_blocking_stream(settings, callback).unwrap();
    audio_stream.start().unwrap();
    
    socket.send_message(&OwnedMessage::Binary(Packet::ImageRequest.serialize())).unwrap();
    
    let mut last_frame = Instant::now();
    let mut last_audio = Instant::now();
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
                            Packet::Ping => {
                                debug!("Ping! {}", socket.peer_addr().unwrap());
                                socket.send_message(&OwnedMessage::Binary(Packet::Pong.serialize())).unwrap()
                            },
                            Packet::ImageResponse(data) => {
                                let compressed_len = data.len();
                                let data = zstd::decode_all(&*data).unwrap();
                                for i in 0..window_buf.len() {
                                    let r = data[(i * 3)];
                                    let g = data[(i * 3) + 1];
                                    let b = data[(i * 3) + 2];
                                    window_buf[i] = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                                }
                                
                                let elapsed = last_frame.elapsed();
                                info!("Last frame received: {:.3}ms | Download FPS: {:.2} | Size: {:.2}KiB vs Compress: {:.2}KiB",
                                    elapsed.as_micros() as f64 / 1000.0,
                                    1.0 / elapsed.as_secs_f64(),
                                    data.len() as f64 / 1024.0,
                                    compressed_len as f64 / 1024.0
                                );
                                last_frame = Instant::now();
                                
                                socket.send_message(&OwnedMessage::Binary(Packet::ImageRequest.serialize())).unwrap();
                            },
                            Packet::AudioSamples(samples) => {
                                let elapsed = last_audio.elapsed();
                                info!("Last audio chunk:   {:.3}ms | Chunk Rate: {:.2}KHz | Size: {:.2}KiB",
                                    elapsed.as_micros() as f64 / 1000.0,
                                    samples.len() as f64 / elapsed.as_secs_f64(),
                                    samples.len() as f64 / 1024.0
                                );
                                last_audio = Instant::now();
                                for sample in samples {
                                    sample_queue.push(sample);
                                }
                            }
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