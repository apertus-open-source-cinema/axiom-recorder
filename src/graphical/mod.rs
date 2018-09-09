extern crate glium;

use glium::*;
use glium::glutin::{WindowBuilder, ContextBuilder, EventsLoop};
use video_io::Image;
use glium::texture::texture2d::Texture2d;
use glium::backend::Facade;
use std::borrow::Cow;
use bus::BusReader;
use graphical::settings::Settings;
use std::time::Duration;
use glium::texture::UncompressedFloatFormat;
use glium::texture::MipmapsOption;

mod settings;
mod gl_util;
mod ui_lib;


/// Manage the rendering process and orchestrate the rendering passes
pub struct Manager {
    display: Display,
    raw_image_source: BusReader<Image>,
    event_loop: EventsLoop,
}

impl Manager {
    pub fn new(raw_image_source: BusReader<Image>) -> Self {
        let event_loop = EventsLoop::new();
        let window = WindowBuilder::new();
        let context = ContextBuilder::new();
        let display = Display::new(window, context, &event_loop).unwrap();

        Manager { display, raw_image_source, event_loop }
    }

    pub fn run_event_loop(&mut self) {
        let mut closed = false;
        while !closed {
            // listing the events produced by application and waiting to be received
            self.event_loop.poll_events(|ev| {
                match ev {
                    glutin::Event::WindowEvent { event, .. } => match event {
                        glutin::WindowEvent::CloseRequested => closed = true,
                        _ => (),
                    },
                    _ => (),
                }
            });

            // look, wether we should debayer a new image
            let gui_settings: Settings = Settings {
                shutter_angle: 0.0,
                iso: 0.0,
                fps: 0.0,
                recording_format: settings::RecordingFormat::RawN,
                grid: settings::Grid::None,
            };

            match self.raw_image_source.recv_timeout(Duration::from_millis(10)) {
                Result::Err(_) => {}
                Result::Ok(image) => {
                    self.redraw(image, &gui_settings);
                }
            }
        }
    }

    pub fn redraw(&self, raw_image: Image, gui_state: &settings::Settings) {
        ui_lib::ColorBox {
            color: [1.0, 0.0, 0.0, 1.0],
            start: (0., 0.),
            size: (1., 1.),
        };
    }
}
