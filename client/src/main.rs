
extern crate env_logger;
#[macro_use] extern crate log;

use std::cmp::max;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use clap::{AppSettings, Arg, Command};
use cpal::{BufferSize, SampleRate, StreamConfig};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_queue::SegQueue;
use log::LevelFilter;
use minifb::{Key, Scale, ScaleMode, Window, WindowOptions};
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
        .arg(Arg::new("verbose")
            .short('v')
            .long("verbose")
            .takes_value(true)
            .default_missing_value("debug")
            .default_value("info")
            .possible_values(["error", "warn", "info", "debug", "trace"])
            .help("Specify the console log level. Environment variable 'RUST_LOG' will override this option."))
        .next_line_help(true)
        .setting(AppSettings::DeriveDisplayOrder)
        .get_matches();
    
    // Setup program-wide logger format
    let level = match std::env::var("RUST_LOG").unwrap_or(matches.value_of("verbose").unwrap_or("info").to_owned()).as_str() {
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
    
    window.limit_update_rate(Some(Duration::from_secs_f32(1.0/15.0)));
    
    let audio_queue = Arc::new(SegQueue::new());
    let callback_audio_queue = audio_queue.clone();
    
    let audio_host = cpal::default_host();
    let audio_device = audio_host.default_output_device().unwrap();
    let config = StreamConfig {
        channels: 2,
        sample_rate: SampleRate(44100),
        buffer_size: BufferSize::Fixed(256),
    };
    let stream = audio_device.build_output_stream(
        &config,
        move |output_buffer: &mut [f32], _info: &cpal::OutputCallbackInfo| {
            for output_sample in output_buffer.iter_mut() {
                if let Some(sample) = callback_audio_queue.pop() {
                    *output_sample = sample;
                } else {
                    *output_sample = 0.0;
                }
            }
        },
        move |_| {
            //
        }
    ).unwrap();
    stream.play().unwrap();
    
    let video_queue = Arc::new(SegQueue::<Vec<u8>>::new());
    let output_video_queue = video_queue.clone();
    
    let frame_endpoint = intercom.endpoint();
    drop(frame_endpoint.send);
    std::thread::spawn(move || {
        while let Ok(msg) = frame_endpoint.recv.recv() {
            match msg {
                InterMessage::BulkFrames(frames) => {
                    debug!("Bulk Received: {}", frames.len());
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
    
    let mut last_request = Instant::now();
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let queue_len = output_video_queue.len();
        if queue_len < 35 && last_request.elapsed() > Duration::from_millis(1000) {
            let remaining = max(35 - queue_len, 20);
            debug!("Framebuffer Health: Local: {} | Requested: {}", queue_len, remaining);
            if queue_len == 0 {
                warn!("Framebuffer is starving...");
            }
            
            if remaining > 0 {
                video_endpoint.send.try_send(InterMessage::SocketPacket(Packet::FrameRequest(remaining as u32))).unwrap();
            }
            
            last_request = Instant::now();
        }
        
        let video = output_video_queue.pop();
        if video.is_none() {
            window.update_with_buffer(&window_buf, WIDTH, HEIGHT).unwrap();
            continue;
        }
        let video = video.unwrap();
        
        
        for i in 0..window_buf.len() {
            let r = video[(i * 3)];
            let g = video[(i * 3) + 1];
            let b = video[(i * 3) + 2];
            window_buf[i] = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        }
        
        window.update_with_buffer(&window_buf, WIDTH, HEIGHT).unwrap();
    }
    
    video_endpoint.send.try_send(InterMessage::Kill).unwrap_or_default();
    std::thread::sleep(Duration::from_secs(1));
}