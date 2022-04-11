
extern crate env_logger;
#[macro_use] extern crate log;

use std::cmp::max;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use clap::{AppSettings, Arg, Command};
use crossbeam_queue::SegQueue;
use log::LevelFilter;
use minifb::{Key, Scale, ScaleMode, Window, WindowOptions};
use websocket::OwnedMessage;
use remote64_common::intercom::{BroadcastNetwork, InterMessage};
use remote64_common::{Feature, Packet};
use crate::socket::SocketManager;


mod socket;


const WIDTH: usize = 720;
const HEIGHT: usize = 480;

fn main() {
    // Run clap to parse cli arguments
    let matches = Command::new("remote64-client")
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
        .arg(Arg::new("domain")
            .long("domain")
            .takes_value(true))
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
    
    // Collect features from cli arguments
    let features: Vec<Feature> = matches.values_of("features").unwrap_or_default().map(|feat| Feature::from_str(feat).unwrap_or_default()).collect();
    
    let mut intercom = BroadcastNetwork::<InterMessage>::new();
    
    // Initialize socket manager which handles the client's connection with the remote64 server
    SocketManager::init(matches.value_of("domain"), features, intercom.endpoint());
    
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
    
    
    pa.is_output_format_supported(output_params, 44100.0).unwrap();
    
    let settings = portaudio::OutputStreamSettings::new(output_params, 44100.0, 512);
    
    let audio_queue = Arc::new(SegQueue::new());
    let callback_audio_queue = audio_queue.clone();
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
            if let Some(sample) = callback_audio_queue.pop() {
                *output_sample = sample;
            } else {
                *output_sample = 0.0;
            }
        }
        
        portaudio::Continue
    };
    
    let mut audio_stream = pa.open_non_blocking_stream(settings, callback).unwrap();
    audio_stream.start().unwrap();
    
    let video_queue = Arc::new(SegQueue::<Vec<u8>>::new());
    let output_video_queue = video_queue.clone();
    
    let frame_endpoint = intercom.endpoint();
    drop(frame_endpoint.send);
    std::thread::spawn(move || {
        while let Ok(msg) = frame_endpoint.recv.recv() {
            match msg {
                InterMessage::BulkFrames(frames) => {
                    println!("Bulk Received: {}", frames.len());
                    for frame in frames {
                        video_queue.push(frame.video);
                        for sample in frame.audio {
                            audio_queue.push(sample);
                        }
                    }
                },
                _ => ()
            }
        }
    });
    
    
    let video_endpoint = intercom.endpoint();
    drop(video_endpoint.recv);
    
    std::thread::spawn(move || {
        intercom.start();
    });
    
    loop {
        video_endpoint.send.try_send(InterMessage::SocketPacket(Packet::FrameRequest(1 as u32))).unwrap();
        
        std::thread::sleep(Duration::from_secs_f64(0.5));
    }
    
    //let mut last_frame = Instant::now();
    //let mut last_audio = Instant::now();
    /*while window.is_open() && !window.is_key_down(Key::Escape) {
        while let Ok(_) = video_endpoint.recv.try_recv() {}
        
        let queue_len = output_video_queue.len();
        if queue_len < 300 {
        println!("test2");
            let remaining = max(15 - queue_len, 5);
            
        println!("test3");
            video_endpoint.send.try_send(InterMessage::SocketPacket(Packet::FrameRequest(25 as u32))).unwrap();
            println!("test333");
            std::thread::sleep(Duration::from_secs_f64(5.0));
        }
        println!("test4");
        
        //let video = output_video_queue.pop();
        let video: Option<Vec<u8>> = Some(vec![127u8; 720*480*3]);
        println!("test5");
        if video.is_none() {
        println!("test6");
            println!("len: {}", window_buf.len());
            window.update_with_buffer(&window_buf, WIDTH, HEIGHT).unwrap();
        println!("test7");
            continue;
        }
        println!("test8");
        let video = video.unwrap();
        
        
        let start = Instant::now();
        for i in 0..window_buf.len() {
            let r = video[(i * 3)];
            let g = video[(i * 3) + 1];
            let b = video[(i * 3) + 2];
            window_buf[i] = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        }
        
        let elapsed = start.elapsed().as_micros() as f64 / 1000.0;
        info!("Frame processing took: {:.3}ms | FPS: {:.2}", elapsed, 1000.0 / elapsed);
        window.update_with_buffer(&window_buf, WIDTH, HEIGHT).unwrap();
    }*/
    
    video_endpoint.send.try_send(InterMessage::Kill).unwrap_or_default();
    std::thread::sleep(Duration::from_secs(1));
}