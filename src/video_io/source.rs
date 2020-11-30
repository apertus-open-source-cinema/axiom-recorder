use crate::util::{
    error::{Res, ResN},
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
                let now = Box::into_raw(Box::new(Instant::now()));
                let result = vs.get_images(&mut |img| {
                    tx.lock().unwrap().broadcast(Arc::new(img));
                    unsafe {
                        // TODO: This is a big, ugly hack
                        println!("{} fps (recv)", 1000 / (*now).elapsed().subsec_millis());
                        now.write(Instant::now());
                    }

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
                    vs: Box::new(Raw8BlobVideoSource { path, width, height, fps, loop_source }),
                });
            } else if path.ends_with(".raw12") {
                return Ok(Self {
                    vs: Box::new(Raw12BlobVideoSource { path, width, height, fps, loop_source }),
                });
            }
        } else {
            // the PathBuf ends_with only considers full childs / path elements
            if entries.iter().all(|p| p.to_str().unwrap().ends_with(".raw8")) {
                return Ok(Self {
                    vs: (Box::new(Raw8FilesVideoSource {
                        files: entries,
                        width,
                        height,
                        fps,
                        loop_source,
                    })),
                });
            } else if entries.iter().all(|p| p.to_str().unwrap().ends_with(".raw12")) {
                return Ok(Self {
                    vs: (Box::new(Raw12FilesVideoSource {
                        files: entries,
                        width,
                        height,
                        fps,
                        loop_source,
                        cache: RefCell::new(BTreeMap::new()),
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
                    vs: (Box::new(TcpVideoSource { address: address.to_string(), width, height })),
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
pub struct Raw8BlobVideoSource {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub fps: Option<f32>,
    pub loop_source: bool,
}

impl VideoSource for Raw8BlobVideoSource {
    fn get_images(&self, callback: &mut dyn FnMut(Image) -> Res<()>) -> Res<()> {
        loop {
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
                    })?;
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
pub struct Raw8FilesVideoSource {
    pub files: Vec<PathBuf>,
    pub width: u32,
    pub height: u32,
    pub fps: Option<f32>,
    pub loop_source: bool,
}

impl VideoSource for Raw8FilesVideoSource {
    fn get_images(&self, callback: &mut dyn FnMut(Image) -> Res<()>) -> Res<()> {
        loop {
            for entry in &self.files {
                let mut file = File::open(entry)?;
                let mut bytes = vec![0u8; (self.width * self.height) as usize];
                file.read_exact(&mut bytes)?;

                let image = Image {
                    width: self.width,
                    height: self.height,
                    bit_depth: 8,
                    data: bytes.clone(),
                };
                callback(image)?;
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

// Reads frames from a single file
pub struct Raw12BlobVideoSource {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub fps: Option<f32>,
    pub loop_source: bool,
}

impl VideoSource for Raw12BlobVideoSource {
    fn get_images(&self, callback: &mut dyn FnMut(Image) -> Res<()>) -> Res<()> {
        loop {
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
                    })?;
                } else if read_size == 0 {
                    // we are at the end of the stream
                    if !self.loop_source {
                        return Ok(());
                    };
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

// Reads a directory of raw12 files
pub struct Raw12FilesVideoSource {
    pub files: Vec<PathBuf>,
    pub width: u32,
    pub height: u32,
    pub fps: Option<f32>,
    pub loop_source: bool,
    pub cache: RefCell<BTreeMap<PathBuf, Image>>,
}

impl VideoSource for Raw12FilesVideoSource {
    fn get_images(&self, callback: &mut dyn FnMut(Image) -> Res<()>) -> Res<()> {
        loop {
            for entry in &self.files {
                let mut cache = self.cache.borrow_mut();
                let image = if cache.contains_key(entry) {
                    (&cache[entry]).clone()
                } else {
                    let mut file = File::open(entry)?;
                    let len = (self.width * self.height + (self.width * self.height / 2)) as usize;
                    let mut bytes_as_raw12 = Vec::with_capacity(len);

                    unsafe {
                        bytes_as_raw12.set_len(len);
                    }

                    // let mut bytes_as_raw8 = vec![0u8; (self.width * self.height) as usize];

                    //            println!("{:?}", bytes);

                    file.read_exact(&mut bytes_as_raw12)?;

                    for i in 0usize..((self.width * self.height / 2) as usize) {
                        let part_a: u16 = bytes_as_raw12[3 * i + 0] as u16;
                        let part_b: u16 = bytes_as_raw12[3 * i + 1] as u16;
                        let part_c: u16 = bytes_as_raw12[3 * i + 2] as u16;

                        let a = ((part_a << 4) & 0xff0) | ((part_b >> 4) | 0xf);
                        let b = ((part_b << 8) & 0xf00) | (part_c | 0xff);

                        fn convert(x: u16) -> u8 {
                            let f: f32 = x as f32;
                            let g = 2.2;

                            ((f / 16.0).powf(g) / (256.0_f32.powf(g - 1.0))) as u8
                        }

                        bytes_as_raw12[2 * i + 0] = convert(a);
                        bytes_as_raw12[2 * i + 1] = convert(b);
                    }

                    bytes_as_raw12.resize((self.width * self.height) as usize, 0);

                    Image {
                        width: self.width,
                        height: self.height,
                        bit_depth: 8,
                        data: bytes_as_raw12,
                    }
                };

                cache.insert(entry.clone(), image.clone());

                callback(image)?;
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
}

impl VideoSource for TcpVideoSource {
    fn get_images(&self, callback: &mut dyn FnMut(Image) -> Res<()>) -> Res<()> {
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

            callback(image)?;
        }
    }

    fn get_frame_count(&self) -> Option<u64> { None }
}
