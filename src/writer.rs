extern crate bus;

use self::bus::BusReader;
use std::thread;
use std::fs::File;
use std::io::prelude::*;
use video_source::Image;

pub struct Writer {
    rx: BusReader<Image>,
    filename: String,
}

impl Writer {
    pub fn new(mut rx: BusReader<Image>, filename: String) {
        thread::spawn(move || {
            let mut file = File::create(filename).unwrap();
    
            loop {
                let img = rx.recv().unwrap();
                file.write_all(&img.data).unwrap();
            }
        });
    }
}
