use super::Image;
use bus::{Bus, BusReader};
use std::{
    fs::File,
    io::prelude::*,
    net::TcpStream,
    rc::Rc,
    sync::{Arc, Mutex},
    thread,
    time::SystemTime,
};

pub trait VideoSource: Send {
    fn get_images(&self, callback: &dyn Fn(Image));
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
                vs.get_images(&|img| {
                    tx.lock().unwrap().broadcast(Arc::new(img));
                })
            });
        }

        BufferedVideoSource { _tx: tx }
    }

    pub fn subscribe(&self) -> BusReader<Arc<Image>> { self._tx.lock().unwrap().add_rx() }
}

// File video source
pub struct FileVideoSource {
    pub path: String,
    pub width: u32,
    pub height: u32,
}

impl VideoSource for FileVideoSource {
    fn get_images(&self, callback: &dyn Fn(Image)) {
        let mut file = File::open(&self.path).unwrap();
        let mut bytes = vec![0u8; (self.width * self.height) as usize];
        file.read_exact(&mut bytes).unwrap();

        loop {
            let image =
                Image { width: self.width, height: self.height, bit_depth: 8, data: bytes.clone() };
            callback(image)
        }
    }
}

#[derive(Debug)]
pub struct EthernetVideoSource {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

impl VideoSource for EthernetVideoSource {
    fn get_images(&self, callback: &dyn Fn(Image)) {
        let mut stream = TcpStream::connect(&self.url).unwrap();

        let mut image_count = 0;
        let mut start = SystemTime::now();

        loop {
            //            let mut bytes = Vec::with_capacity((self.width * self.height) as
            // usize);
            let mut bytes = vec![0u8; (self.width * self.height) as usize];

            stream.read_exact(&mut bytes).unwrap();

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
