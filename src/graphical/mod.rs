extern crate glium;

use bus::BusReader;
use glium::backend::Facade;
use glium::glutin::{ContextBuilder, EventsLoop, WindowBuilder};
use glium::texture::texture2d::Texture2d;
use glium::texture::MipmapsOption;
use glium::texture::UncompressedFloatFormat;
use glium::*;
use graphical::settings::Settings;
use graphical::ui_lib::Drawable;
use graphical::ui_lib::Pos;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::time::Duration;
use std::time::Instant;
use video_io::Image;

mod gl_util;
mod settings;
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

        Manager {
            display,
            raw_image_source,
            event_loop,
        }
    }

    pub fn run_event_loop(&mut self) {
        let mut closed = false;
        while !closed {
            let now = Instant::now();
            // listing the events produced by application and waiting to be received
            self.event_loop.poll_events(|ev| match ev {
                glutin::Event::WindowEvent { event, .. } => match event {
                    glutin::WindowEvent::CloseRequested => closed = true,
                    _ => (),
                },
                _ => (),
            });

            // look, wether we should debayer a new image
            let gui_settings: Settings = Settings {
                shutter_angle: 0.0,
                iso: 0.0,
                fps: 0.0,
                recording_format: settings::RecordingFormat::RawN,
                grid: settings::Grid::None,
            };

            match self
                .raw_image_source
                .recv_timeout(Duration::from_millis(10))
            {
                Result::Err(_) => {}
                Result::Ok(image) => {
                    self.redraw(image, &gui_settings);
                }
            }

            println!("{} fps", 1000 / now.elapsed().subsec_millis());
        }
    }

    pub fn redraw(&mut self, raw_image: Image, gui_state: &settings::Settings) {
        let mut target = self.display.draw();
        target.clear_color(0.0, 0.0, 0.0, 0.0);

        vec![
            (
                &ui_lib::debayer::Debayer { raw_image } as &Drawable,
                Pos::full(),
            ),
            (
                &ui_lib::util::ColorBox {
                    color: [0.0, 0.0, 0.0, 0.5],
                } as &Drawable,
                Pos {
                    start: (0., 0.),
                    size: (1., 0.1),
                },
            ),
        ].draw(
            &mut (&mut target, &mut self.display, &mut BTreeMap::new()),
            Pos::full(),
        );

        target.finish();
    }
}
