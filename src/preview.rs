extern crate gdk;
extern crate spmc;

use video_source::Image;

pub struct OpenGlHandler {
    pub context: gdk::GLContext,
    pub source: spmc::Receiver<Image>,
}

impl OpenGlHandler {
    pub fn new(context: gdk::GLContext, source: spmc::Receiver<Image>) {}

    pub fn start(&self) {}
}
