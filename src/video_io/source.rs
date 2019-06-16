use super::Image;
use bus::{Bus, BusReader};
use std::{
    error,
    fs::{self, File},
    io::prelude::*,
    net::TcpStream,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::SystemTime,
};

type Res = Result<(), Box<error::Error>>;

pub trait VideoSource: Send {
    fn get_images(&self, callback: &dyn Fn(Image)) -> Res;
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
                let result = vs.get_images(&|img| {
                    tx.lock().unwrap().broadcast(Arc::new(img));
                });

                if result.is_err() {
                    eprintln!("{}", result.err().unwrap());
                }
            });
        }

        BufferedVideoSource { _tx: tx }
    }

    pub fn subscribe(&self) -> BusReader<Arc<Image>> { self._tx.lock().unwrap().add_rx() }
}

// File video source
pub struct Raw8FileVideoSource {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub repeat: bool,
}

impl VideoSource for Raw8FileVideoSource {
    fn get_images(&self, callback: &dyn Fn(Image)) -> Res {
        let mut file = File::open(&self.path)?;
        let mut bytes = vec![0u8; (self.width * self.height) as usize];
        file.read_exact(&mut bytes)?;

        loop {
            let image =
            Image { width: self.width, height: self.height, bit_depth: 8, data: bytes.clone() };
            callback(image);
            if !self.repeat { return Ok(()); }
        }
    }
}

// File video source
pub struct Raw8FilesVideoSource {
    pub folder_path: String,
    pub width: u32,
    pub height: u32,
}

impl VideoSource for Raw8FilesVideoSource {
    fn get_images(&self, callback: &dyn Fn(Image)) -> Res {
        let path = Path::new(&self.folder_path);
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let mut file = File::open(entry.path())?;
            let mut bytes = vec![0u8; (self.width * self.height) as usize];
            file.read_exact(&mut bytes)?;

            let image =
                Image { width: self.width, height: self.height, bit_depth: 8, data: bytes.clone() };
            callback(image)
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct EthernetVideoSource {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

impl VideoSource for EthernetVideoSource {
    fn get_images(&self, callback: &dyn Fn(Image)) -> Res {
        let mut stream = TcpStream::connect(&self.url)?;

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
