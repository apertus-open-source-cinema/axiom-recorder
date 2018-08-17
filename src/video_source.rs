use std::thread;
extern crate spmc;

use std::fs::File;
use std::io::prelude::*;

use std::marker;

// general stuff
#[derive(Debug)]
pub struct Image {
    width: u32,
    height: u32,
    bit_depth: u8,

    data: Vec<u8>,
}

trait VideoSource {
    fn get_images(&self, &Fn(Image));
}

#[derive(Debug)]
struct BufferedVideoSource<VS> {
    _marker: marker::PhantomData<VS>,
    _rx: spmc::Receiver<Image>,
}
impl<T> BufferedVideoSource<T>
where
    T: VideoSource + marker::Send + 'static,
{
    fn new(vs: T) -> BufferedVideoSource<T> {
        let (tx, rx) = spmc::channel();
        let handle = thread::spawn(move || {
            vs.get_images(&|img| {
                tx.send(img);
            })
        });

        BufferedVideoSource {
            _rx: rx,
            _marker: marker::PhantomData,
        }
    }

    fn subscribe(&self) -> spmc::Receiver<Image> {
        self._rx.clone()
    }
}

// File video source
#[derive(Debug)]
struct FileVideoSource {
    path: String,
    width: u32,
    height: u32,
    bit_depth: u8,
}
impl VideoSource for FileVideoSource {
    fn get_images(&self, callback: &Fn(Image)) {
        let mut file = File::open(&self.path).unwrap();
        let mut bytes = Vec::with_capacity(file.metadata().unwrap().len() as usize);
        file.read_to_end(&mut bytes).unwrap();

        let image = Image {
            width: self.width,
            height: self.height,
            bit_depth: 8,
            data: bytes,
        };
        callback(image)
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
