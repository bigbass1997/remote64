use std::path::Path;
use v4l::{Device, Format, FourCC};
use v4l::format::{Colorspace, FieldOrder};
use v4l::video::Capture;


pub const SUPPORTED_FOURCC: [[u8; 4]; 1] = [*b"RGBP",];


pub enum Error {
    IoError(std::io::Error),
    NoDeviceFound,
    NoSupportedColorFormat,
}
use Error::*;

pub struct VideoStream {
    dev: Device,
}
impl VideoStream {
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
            Ok(dev) => Ok(Self {
                dev,
            }),
            Err(err) => Err(IoError(err))
        }
    }
    
    pub fn with(dev: Device) -> Self {
        Self {
            dev,
        }
    }
    
    pub fn new() -> Result<Self, Error> {
        let mut devices = Self::devices();
        if devices.is_empty() {
            return Err(NoDeviceFound);
        }
        
        let mut dev = devices.remove(0);
        let fmt = init_fmt(&mut dev);
        if !SUPPORTED_FOURCC.contains(&fmt.fourcc.repr) {
            return Err(NoSupportedColorFormat);
        }
        
        Ok(Self {
            dev: devices.remove(0),
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