extern crate gdk;
extern crate spmc;
use std::thread;
use video_source::Image;


pub struct OpenGlHandler {
    pub context: gdk::GLContext,
    pub source: spmc::Receiver<Image>,
}

impl OpenGlHandler {
    pub fn start(&self) {
        thread::spawn(move || {});
    }
}
