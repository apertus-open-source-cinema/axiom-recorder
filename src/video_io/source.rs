use crate::util::{
    error::{Res, ResN},
    fps_report::FPSReporter,
    image::Image,
    options::OptionsStorage,
};
use bus::{Bus, BusReader};
use glob::glob;
use itertools::Itertools;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs::File,
    io::{prelude::*, Error, ErrorKind},
    net::TcpStream,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread::{self, sleep},
    time::{Duration, Instant, SystemTime},
};

pub trait VideoSource: Send {
    fn get_images(&self, callback: &mut dyn FnMut(Image) -> Res<()>) -> ResN;
    fn get_frame_count(&self) -> Option<u64>;
}

pub struct BufferedVideoSource {
    tx: Arc<Mutex<Bus<Arc<Image>>>>,
}

impl BufferedVideoSource {
    pub fn new(vs: Box<dyn VideoSource>) -> BufferedVideoSource {
        let tx = Bus::new(10);

        let tx = Arc::new(Mutex::new(tx));
        let vs_send = Arc::new(Mutex::new(vs));

        {
            let tx = tx.clone();
            thread::spawn(move || {
                let vs = vs_send.lock().unwrap();
                let mut fps_reporter = FPSReporter::new("source");
                let result = vs.get_images(&mut |img| {
                    drop(img.buffer.u8_buffer());
                    tx.lock().unwrap().broadcast(Arc::new(img));
                    fps_reporter.frame();
                    Ok(())
                });

                if result.is_err() {
                    eprintln!("Source Error: {}", result.err().unwrap());
                    return;
                }
            });
        }

        BufferedVideoSource { tx }
    }

    pub fn subscribe(&self) -> BusReader<Arc<Image>> { self.tx.lock().unwrap().add_rx() }
}

pub struct MetaVideoSource {
    vs: Box<dyn VideoSource>,
}

impl MetaVideoSource {
    pub fn from_file(path: String, options: &OptionsStorage) -> Res<Self> {
        let width = options.get_opt_parse("width")?;
        let height = options.get_opt_parse("height")?;
        let fps = options.get_opt_parse("fps").ok();
        let loop_source = options.is_present("loop");

        let entries = glob(&path)?.collect::<Result<Vec<PathBuf>, _>>()?;

        if entries.len() == 1 {
            if path.ends_with(".raw8") {
                return Ok(Self {
                    vs: Box::new(RawNBlobVideoSource {
                        path,
                        bit_depth: 8,
                        width,
                        height,
                        fps,
                        loop_source,
                    }),
                });
            } else if path.ends_with(".raw12") {
                return Ok(Self {
                    vs: Box::new(RawNBlobVideoSource {
                        path,
                        bit_depth: 12,
                        width,
                        height,
                        fps,
                        loop_source,
                    }),
                });
            }
        } else {
            // the PathBuf ends_with only considers full childs / path elements
            if entries.iter().all(|p| p.to_str().unwrap().ends_with(".raw8")) {
                return Ok(Self {
                    vs: (Box::new(RawNDirectoryVideoSource {
                        files: entries,
                        bit_depth: 8,
                        width,
                        height,
                        fps,
                        loop_source,
                    })),
                });
            } else if entries.iter().all(|p| p.to_str().unwrap().ends_with(".raw12")) {
                return Ok(Self {
                    vs: (Box::new(RawNDirectoryVideoSource {
                        files: entries,
                        bit_depth: 12,
                        width,
                        height,
                        fps,
                        loop_source,
                    })),
                });
            }
        }

        Err(Box::new(Error::new(ErrorKind::InvalidData, "file type is not supported")))
    }

    pub fn from_uri(uri: String, options: &OptionsStorage) -> Res<Self> {
        match uri
            .split("://")
            .next_tuple()
            .ok_or(Error::new(ErrorKind::InvalidInput, "malformad URI"))?
        {
            ("file", path) => Ok(Self::from_file(path.to_string(), options)?),
            ("tcp", address) => {
                let width = options.get_opt_parse("width")?;
                let height = options.get_opt_parse("height")?;

                Ok(Self {
                    vs: (Box::new(TcpVideoSource {
                        address: address.to_string(),
                        width,
                        height,
                        bit_depth: 8,
                    })),
                })
            }
            (uri_type, _) => Err(Box::new(Error::new(
                ErrorKind::InvalidInput,
                format!("URI type {} is not supported", uri_type),
            ))),
        }
    }
}

impl VideoSource for MetaVideoSource {
    fn get_images(&self, callback: &mut dyn FnMut(Image) -> Res<()>) -> Res<()> {
        self.vs.get_images(callback)
    }

    fn get_frame_count(&self) -> Option<u64> { self.vs.get_frame_count() }
}

// Reads frames from a single file
pub struct RawNBlobVideoSource {
    pub path: String,
    pub bit_depth: u8,
    pub width: u32,
    pub height: u32,
    pub fps: Option<f32>,
    pub loop_source: bool,
}

impl VideoSource for RawNBlobVideoSource {
    fn get_images(&self, callback: &mut dyn FnMut(Image) -> Res<()>) -> Res<()> {
        loop {
            let mut file = File::open(&self.path)?;
            loop {
                let mut bytes =
                    vec![0u8; (self.width * self.height * self.bit_depth as u32 / 8) as usize];
                let read_size = file.read(&mut bytes)?;

                if read_size == bytes.len() {
                    callback(Image::new(self.width, self.height, bytes, self.bit_depth)?)?;
                } else if read_size == 0 {
                    // we are at the end of the stream
                    if !self.loop_source {
                        return Ok(());
                    }
                } else {
                    return Err(Box::new(Error::new(
                        ErrorKind::InvalidData,
                        "File could not be fully consumed. is the resolution set right?",
                    )));
                }
                if self.fps.is_some() {
                    sleep(Duration::from_millis((1000.0 / self.fps.unwrap()) as u64))
                }
            }
        }
    }

    fn get_frame_count(&self) -> Option<u64> {
        Some(Path::new(&self.path).metadata().unwrap().len() / ((self.width * self.height) as u64))
    }
}

// Reads a directory of raw8 files
pub struct RawNDirectoryVideoSource {
    pub files: Vec<PathBuf>,
    pub bit_depth: u8,
    pub width: u32,
    pub height: u32,
    pub fps: Option<f32>,
    pub loop_source: bool,
}

impl VideoSource for RawNDirectoryVideoSource {
    fn get_images(&self, callback: &mut dyn FnMut(Image) -> Res<()>) -> Res<()> {
        loop {
            for entry in &self.files {
                let mut file = File::open(entry)?;
                let mut bytes =
                    vec![0u8; (self.width * self.height * self.bit_depth as u32 / 8) as usize];
                file.read_exact(&mut bytes)?;

                callback(Image::new(self.width, self.height, bytes, self.bit_depth)?)?;
                if self.fps.is_some() {
                    sleep(Duration::from_millis((1000.0 / self.fps.unwrap()) as u64));
                }
            }

            if !self.loop_source {
                return Ok(());
            }
        }
    }

    fn get_frame_count(&self) -> Option<u64> { Some(self.files.len() as u64) }
}

// Reads frames from a remote connected camera
pub struct TcpVideoSource {
    pub address: String,
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
}

impl VideoSource for TcpVideoSource {
    fn get_images(&self, callback: &mut dyn FnMut(Image) -> Res<()>) -> Res<()> {
        let mut stream = TcpStream::connect(&self.address)?;
        let mut fps_reporter = FPSReporter::new("video source");

        loop {
            let mut bytes =
                vec![0u8; (self.width * self.height * self.bit_depth as u32 / 8) as usize];
            stream.read_exact(&mut bytes)?;
            let image = Image::new(self.width, self.height, bytes, self.bit_depth)?;
            callback(image)?;
            fps_reporter.frame()
        }
    }

    fn get_frame_count(&self) -> Option<u64> { None }
}
