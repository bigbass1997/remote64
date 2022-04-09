
use std::io::BufWriter;
use std::fs::File;
use std::path::{Path, PathBuf};
use hound::{Sample, WavSpec, WavWriter};
use image::RgbImage;

pub const WAV_PATH: &'static str = "recording/audio.wav";

pub struct Recording {
    wav_writer: WavWriter<BufWriter<File>>,
    img: RgbImage,
    
    frame_index: u32,
    started: bool,
}
impl Recording {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            wav_writer: get_wav_writer(WAV_PATH, 2, 44100.0).unwrap(),
            img: RgbImage::new(width, height),
            frame_index: 0,
            started: false,
        }
    }
    
    /// Starts the recording.
    /// 
    /// If recording was already started, it must be ended otherwise this does nothing.
    /// 
    /// When started, the framebuffer is cleared, frame counter reset to 0, and audio writer restarted.
    pub fn start(&mut self) {
        if self.started { return }
        
        self.wav_writer = get_wav_writer(WAV_PATH, 2, 44100.0).unwrap();
        self.img.fill(0);
        self.frame_index = 0;
        
        self.started = true;
    }
    
    /// Saves the current frame data, and increments the internal frame counter.
    /// 
    /// Recording must have been started, otherwise this does nothing.
    pub fn frame(&mut self) {
        if !self.started { return }
        
        self.img.save(format!("recording/output-{:08}.bmp", self.frame_index)).unwrap();
        
        self.frame_index += 1;
    }
    
    pub fn set_pixel_i(&mut self, i: u32, r: u8, g: u8, b: u8) {
        let width = self.img.width();
        let x = i % width;
        let y = i / width;
        
        self.set_pixel_xy(x, y, r, g, b);
    }
    
    pub fn set_pixel_xy(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8) {
        (*self.img.get_pixel_mut(x, y)).0 = [r, g, b];
    }
    
    pub fn sample<S: Sample>(&mut self, sample: S) {
        if !self.started { return }
        
        self.wav_writer.write_sample(sample).unwrap();
    }
    
    pub fn started(&self) -> bool { self.started }
    
    pub fn end(&mut self) {
        if !self.started { return }
        
        self.started = false;
        
        self.wav_writer.flush().unwrap();
    }
}

fn get_wav_writer(path: &'static str, channels: i32, sample_rate: f64) -> Result<WavWriter<BufWriter<File>>, String> {
    if let Some(parent) = Path::new(path).parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    
    let spec = wav_spec(channels, sample_rate);
    match WavWriter::create(path, spec) {
        Ok(writer) => Ok(writer),
        Err(error) => Err (String::from(format!("{}",error))),
    }
}

fn wav_spec(channels: i32, sample_rate: f64) -> WavSpec {
    WavSpec {
        channels: channels as _,
        sample_rate: sample_rate as _,
        bits_per_sample: (std::mem::size_of::<f32>() * 8) as _,
        sample_format: hound::SampleFormat::Float,
    }
}