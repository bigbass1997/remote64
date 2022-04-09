use std::path::Path;
use v4l::{Device, Format, FourCC};
use v4l::buffer::Type;
use v4l::format::{Colorspace, FieldOrder};
use v4l::video::Capture;
use v4l::io::mmap::Stream;


pub const SUPPORTED_FOURCC: [[u8; 4]; 1] = [*b"RGBP",];

#[derive(Debug)]
pub enum Error {
    IoError(std::io::Error),
    NoDeviceFound,
    NoSupportedColorFormat,
}
use Error::*;

pub struct VideoStream<'a> {
    pub dev: Device,
    pub stream: Stream<'a>,
}
impl<'a> VideoStream<'a> {
    pub fn devices() -> Vec<Device> {
        let mut devices = vec![];
        
        for i in 0..20 {
            match Device::new(i) {
                Ok(dev) => devices.push(dev),
                Err(_) => ()
            }
        }
        
        devices
    }
    
    pub fn with_path<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        match Device::with_path(path) {
            Ok(dev) => {
                let stream = Stream::with_buffers(&dev, Type::VideoCapture, 4).unwrap();
                
                Ok(Self {
                    dev,
                    stream,
                })
            },
            Err(err) => Err(IoError(err))
        }
    }
    
    pub fn with(dev: Device) -> Self {
        let stream = Stream::with_buffers(&dev, Type::VideoCapture, 4).unwrap();
        
        Self {
            dev,
            stream,
        }
    }
    
    pub fn new() -> Result<Self, Error> {
        let mut devices = Self::devices();
        if devices.is_empty() {
            return Err(NoDeviceFound);
        }
        
        let mut dev = devices.remove(0);
        let fmt = init_fmt(&mut dev);
        debug!("Capture device format:\n{}", fmt);
        if !SUPPORTED_FOURCC.contains(&fmt.fourcc.repr) {
            return Err(NoSupportedColorFormat);
        }
        
        let stream = Stream::with_buffers(&dev, Type::VideoCapture, 4).unwrap();
        
        Ok(Self {
            dev,
            stream,
        })
    }
    
    /// Attempts to resize the resolution of the captured video.
    /// 
    /// Returns the resulting format.
    pub fn resize(&mut self, width: u32, height: u32) -> Format {
        let mut fmt = self.dev.format().unwrap();
        fmt.width = width;
        fmt.height = height;
        
        self.dev.set_format(&fmt).unwrap()
    }
    
    /// Sets the maximum possible resolution of the captured video.
    pub fn resize_max(&mut self) -> Format {
        self.resize(u32::MAX, u32::MAX)
    }
}

fn init_fmt(dev: &mut Device) -> Format {
    let mut fmt = dev.format().unwrap();
    fmt.width = u32::MAX;
    fmt.height = u32::MAX;
    fmt.fourcc = FourCC::new(b"RGBP");
    fmt.colorspace = Colorspace::NTSC;
    fmt.field_order = FieldOrder::Alternate;
    
    dev.set_format(&fmt).unwrap()
}