use std::thread;
extern crate bus;

use std::fs::File;
use std::io::prelude::*;
use std::net::TcpStream;
use std::time::SystemTime;
use self::bus::{Bus, BusReader};
use std::sync::{Arc, Mutex};

use std::marker;

// general stuff
#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,

    pub data: Vec<u8>,
}

trait VideoSource {
    fn get_images(&self, &Fn(Image));
}

// #[derive(Debug)]
pub struct BufferedVideoSource<VS> {
    _marker: marker::PhantomData<VS>,
    _tx: Arc<Mutex<Bus<Image>>>,
}
impl<T> BufferedVideoSource<T>
where
    T: VideoSource + marker::Send + 'static,
{
    pub fn new(vs: T) -> BufferedVideoSource<T> {
        let mut tx = Bus::new(30 * 10); // 10 seconds footage

        let tx = Arc::new(Mutex::new(tx));

        {
            let tx = tx.clone();
            thread::spawn(move || vs.get_images(&|img| { tx.lock().unwrap().broadcast(img); }));
        }

        BufferedVideoSource {
            _tx: tx,
            _marker: marker::PhantomData,
        }
    }

    pub fn subscribe(&self) -> BusReader<Image> {
        self._tx.lock().unwrap().add_rx()
    }
}

// File video source
#[derive(Debug)]
pub struct FileVideoSource {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
}

impl VideoSource for FileVideoSource {
    fn get_images(&self, callback: &Fn(Image)) {
        let mut file = File::open(&self.path).unwrap();


        loop {
        let mut bytes = vec![0u8; (self.width * self.height) as usize];
            file.read_exact(&mut bytes).unwrap();

            let image = Image {
                width: self.width,
                height: self.height,
                bit_depth: 8,
                data: bytes,
            };
            callback(image)
        }
    }
}

#[derive(Debug)]
pub struct EthernetVideoSource {
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
}

impl VideoSource for EthernetVideoSource {
    fn get_images(&self, callback: &Fn(Image)) {
        let mut stream = TcpStream::connect(&self.url).unwrap();

        let mut image_count = 0;
        let mut start = SystemTime::now();

        
        loop {
//            let mut bytes = Vec::with_capacity((self.width * self.height) as usize);
            let mut bytes = vec![0u8; (self.width * self.height) as usize];


            stream.read_exact(&mut bytes).unwrap();

            let image = Image {
                width: self.width,
                height: self.height,
                bit_depth: 8,
                data: bytes,
            };

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

/*
// USB3
struct Usb3VideoSource {
    size : Option<(u32, u32)>,
}

impl VideoSource for Usb3VideoSource {
    fn get_image(mut self, callbackFunction: fn(Image)) {

    }

    fn get_size(&self) -> (u32, u32) {
        (0, 0)
    }
}


// Ethernet
struct EthernetVideoSource {
    url: String,

    _rx: spmc::Receiver<Image>,
    _tx: spmc::Sender<Image>,
}

impl EthernetVideoSource {
    fn new(&self, host: String) -> EthernetVideoSource {
        let (tx, rx) = spmc::channel();
        let instance = EthernetVideoSource {url: host, _rx: rx, _tx: tx};

        thread::spawn(move || {
            
        });

        instance
    }
}

impl VideoSource for EthernetVideoSource {
    fn get_image(self, callback: fn(Image)) -> spmc::Receiver<Image> {
        self._rx.clone()
    }

    fn get_size(&self) -> (u32, u32) {
        (0, 0)
    }
}
*/
