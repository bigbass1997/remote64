
extern crate env_logger;
#[macro_use] extern crate log;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use clap::{AppSettings, Arg, Command};
use crossbeam_queue::SegQueue;
use log::LevelFilter;
use minifb::{Key, Window, WindowOptions};
use minifb::{Scale, ScaleMode};
use portaudio::DeviceIndex;
use v4l::io::traits::OutputStream;
use remote64_common::{Feature, Frame};
use remote64_common::util::InfCell;
use remote64_common::intercom::{BroadcastNetwork, InterMessage};
use crate::sockets::SocketManager;
use crate::recording::Recording;
use crate::video::VideoStream;


mod sockets;
mod recording;
mod video;


const WIDTH: usize = 720;
const HEIGHT: usize = 480;

fn main() {
    // Run clap to parse cli arguments
    let matches = Command::new("remote64-server")
        .version(clap::crate_version!())
        .arg(Arg::new("verbose")
            .short('v')
            .long("verbose")
            .takes_value(true)
            .default_missing_value("debug")
            .default_value("info")
            .possible_values(["error", "warn", "info", "debug", "trace"])
            .help("Specify the console log level. Environment variable 'RUST_LOG' will override this option."))
        .arg(Arg::new("features")
            .short('f')
            .long("feature")
            .takes_value(true)
            .multiple_occurrences(true)
            .possible_values(["LivePlayback", "AudioRecording", "InputHandling"])
            .help("Specify a feature supported by this server. Use multiple -f/--feature args to specify multiple features."))
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
    
    // Initialize socket manager which handles the client connections and request queue
    SocketManager::init(features, intercom.endpoint());
    
    
    
    
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
    
    let recording = InfCell::new(Recording::new(WIDTH as u32, HEIGHT as u32));
    let audio_recording = recording.get_mut();
    let video_recording = recording.get_mut();
    for _ in 0..15 {
        video_recording.frame(); // delay video by 15 frames to better sync with audio recording
    }
    
    let manage_recording = recording.get_mut();
    let recording_endpoint = intercom.endpoint();
    std::thread::spawn(move || {
        while let Ok(msg) = recording_endpoint.recv.recv() {
            match msg {
                InterMessage::StartRecording => {
                    info!("Recording started.");
                    manage_recording.start();
                },
                InterMessage::StopRecording => {
                    info!("Recording ended.");
                    manage_recording.end();
                },
                _ => ()
            }
        }
        info!("Recording endpoint died.");
    });
    
    
    let audio_endpoint = intercom.endpoint();
    drop(audio_endpoint.recv);
    let samples = Arc::new(SegQueue::<f32>::new());
    let callback_samples = samples.clone();
    let callback = move |portaudio::stream::DuplexCallbackArgs {
                             in_buffer,
                             out_buffer,
                             frames: _,
                             flags, 
                             time: _,
                         }| {
        if !flags.is_empty() {
            debug!("flags: {:?}", flags);
        }
        
        for (output_sample, input_sample) in out_buffer.iter_mut().zip(in_buffer.iter()) {
            *output_sample = *input_sample;
            
            callback_samples.push(*input_sample);
            
            if audio_recording.started() {
                audio_recording.sample(*input_sample);
            }
        }
        
        portaudio::Continue
    };
    
    let mut audio_stream = pa.open_non_blocking_stream(settings, callback).unwrap();
    audio_stream.start().unwrap();
    
    
    
    
    let video_endpoint = intercom.endpoint();
    drop(video_endpoint.recv);
    std::thread::spawn(move || {
        intercom.start();
    });
    
    let mut video_capture = VideoStream::new().unwrap(); //TODO allow server user to specify which device to use
    
    window_buf.fill(0);
    let mut socket_buf = vec![0; window_buf.len() * 3];
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let (stream_buf, _meta) = video_capture.stream.next().unwrap(); // blocks until next frame, thus may limit FPS
        
        // decode stream buffer and distribute among other framebuffers
        for i in (0..stream_buf.len()).step_by(2) { // assumes RGBP format, which uses 2 bytes per pixel
            let r = stream_buf[i + 1] & 0b11111000;
            let g = ((stream_buf[i + 1] & 0b00000111) << 5) | ((stream_buf[i] & 0b11100000) >> 3);
            let b = (stream_buf[i] & 0b00011111) << 3;
            let color = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
            
            let i = i / 2;
            
            // update server's framebuffer (u32 0RGB)
            window_buf[i] = color;
            
            // update client's framebuffer (u8 u8 u8 RGB)
            socket_buf[(i * 3) + 0] = r;
            socket_buf[(i * 3) + 1] = g;
            socket_buf[(i * 3) + 2] = b;
            
            // update video recording framebuffer ([u8; 3] RGB)
            video_recording.set_pixel_i(i as u32, r, g, b);
        }
        
        // update server window
        window.update_with_buffer(&window_buf, WIDTH, HEIGHT).unwrap();
        
        // attempt to save new video frame (if recording is running)
        video_recording.frame();
        
        
        // send latest frame
        let len = samples.len();
        let mut sample_buf = Vec::with_capacity(len);
        for _ in 0..len {
            sample_buf.push(samples.pop().unwrap());
        }
        video_endpoint.send.try_send(InterMessage::LatestFrame(Frame::new(socket_buf.clone(), sample_buf))).unwrap_or_default();
    }
    
    audio_stream.stop().unwrap();
    
    recording.get_mut().end();
}