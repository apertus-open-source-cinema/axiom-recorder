use super::Image;
use bus::{Bus, BusReader};
use glium::framebuffer::ValidationError;
use itertools::Itertools;
use std::{
    error,
    fs::{self, File},
    io::{prelude::*, Error, ErrorKind},
    net::TcpStream,
    path::Path,
    sync::{Arc, Mutex},
    thread::{self, sleep_ms},
    time::{Instant, SystemTime},
};

type Res<a> = Result<a, Box<error::Error>>;

pub trait VideoSource: Send {
    fn get_images(&self, callback: &dyn Fn(Image)) -> Res<()>;
}

pub struct BufferedVideoSource {
    _tx: Arc<Mutex<Bus<Arc<Image>>>>,
}

impl BufferedVideoSource {
    pub fn new(vs: Box<dyn VideoSource>) -> BufferedVideoSource {
        let tx = Bus::new(30); // 1 second footage @30fps

        let tx = Arc::new(Mutex::new(tx));
        let vs_send = Arc::new(Mutex::new(vs));

        {
            let tx = tx.clone();
            thread::spawn(move || {
                let vs = vs_send.lock().unwrap();
                let now = Box::into_raw(Box::new(Instant::now()));
                let result = vs.get_images(&|img| {
                    tx.lock().unwrap().broadcast(Arc::new(img));
                    unsafe {
                        // TODO: This is a big, ugly hack
                        println!("{} fps (recv)", 1000 / (*now).elapsed().subsec_millis());
                        now.write(Instant::now());
                    }
                });

                if result.is_err() {
                    eprintln!("Source Error: {}", result.err().unwrap());
                }
            });
        }

        BufferedVideoSource { _tx: tx }
    }

    pub fn from_file(
        path: String,
        width: u32,
        height: u32,
        fps: Option<f32>,
    ) -> Res<BufferedVideoSource> {
        if path.ends_with(".raw8") {
            Ok(Self::new(Box::new(Raw8BlobVideoSource {
                path: path.to_string(),
                width,
                height,
                fps,
            })))
        } else if Path::new(&path).is_dir() {
            Ok(Self::new(Box::new(Raw8FilesVideoSource {
                folder_path: path.to_string(),
                width,
                height,
                fps,
            })))
        } else {
            Err(Box::new(Error::new(ErrorKind::InvalidData, "file type is not supported")))
        }
    }

    pub fn from_uri(
        uri: String,
        width: u32,
        height: u32,
        fps: Option<f32>,
    ) -> Res<BufferedVideoSource> {
        match uri
            .split("://")
            .next_tuple()
            .ok_or(Error::new(ErrorKind::InvalidInput, "malformad URI"))?
        {
            ("file", path) => Ok(Self::from_file(path.to_string(), width, height, fps)?),
            ("tcp", address) => Ok(Self::new(Box::new(TcpVideoSource {
                address: address.to_string(),
                width,
                height,
            }))),
            (uri_type, _) => Err(Box::new(Error::new(
                ErrorKind::InvalidInput,
                format!("URI type {} is not supported", uri_type),
            ))),
        }
    }


    pub fn subscribe(&self) -> BusReader<Arc<Image>> { self._tx.lock().unwrap().add_rx() }
}

// Reads frames from a single file
pub struct Raw8BlobVideoSource {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub fps: Option<f32>,
}

impl VideoSource for Raw8BlobVideoSource {
    fn get_images(&self, callback: &dyn Fn(Image)) -> Res<()> {
        let mut file = File::open(&self.path)?;
        let mut bytes = vec![0u8; (self.width * self.height) as usize];

        loop {
            let read_size = file.read(&mut bytes)?;

            if read_size == bytes.len() {
                callback(Image {
                    width: self.width,
                    height: self.height,
                    bit_depth: 8,
                    data: bytes.clone(),
                });
            } else if read_size == 0 {
                // we are at the end of the stream
                return Ok(());
            } else {
                return Err(Box::new(Error::new(
                    ErrorKind::InvalidData,
                    "File could not be fully consumed. is the resolution set right?",
                )));
            }
            if self.fps.is_some() {
                sleep_ms((1000.0 / self.fps.unwrap()) as u32)
            }
        }
    }
}

// Reads a directory of raw8 files
pub struct Raw8FilesVideoSource {
    pub folder_path: String,
    pub width: u32,
    pub height: u32,
    pub fps: Option<f32>,
}

impl VideoSource for Raw8FilesVideoSource {
    fn get_images(&self, callback: &dyn Fn(Image)) -> Res<()> {
        let path = Path::new(&self.folder_path);
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let mut file = File::open(entry.path())?;
            let mut bytes = vec![0u8; (self.width * self.height) as usize];
            file.read_exact(&mut bytes)?;

            let image =
                Image { width: self.width, height: self.height, bit_depth: 8, data: bytes.clone() };
            callback(image);
            if self.fps.is_some() {
                sleep_ms((1000.0 / self.fps.unwrap()) as u32);
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct TcpVideoSource {
    pub address: String,
    pub width: u32,
    pub height: u32,
}

impl VideoSource for TcpVideoSource {
    fn get_images(&self, callback: &dyn Fn(Image)) -> Res<()> {
        let mut stream = TcpStream::connect(&self.address)?;

        let mut image_count = 0;
        let mut start = SystemTime::now();

        loop {
            //            let mut bytes = Vec::with_capacity((self.width * self.height) as
            // usize);
            let mut bytes = vec![0u8; (self.width * self.height) as usize];

            stream.read_exact(&mut bytes)?;

            let image = Image { width: self.width, height: self.height, bit_depth: 8, data: bytes };

            let time = SystemTime::now().duration_since(start).expect("Time went backwards");
            if time.as_secs() > 1 {
                let in_ms = time.as_secs() * 1000 + time.subsec_nanos() as u64 / 1_000_000;

                println!("{} fps", ((image_count as f64) / (in_ms as f64)) * 1000.0);

                image_count = 0;
                start = SystemTime::now();
            } else {
                image_count += 1;
            }

            callback(image)
        }
    }
}
