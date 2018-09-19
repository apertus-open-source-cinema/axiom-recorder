extern crate glium;

use bus::BusReader;
use glium::backend::Facade;
use glium::glutin::{ContextBuilder, EventsLoop, WindowBuilder};
use glium::texture::texture2d::Texture2d;
use glium::texture::MipmapsOption;
use glium::texture::UncompressedFloatFormat;
use glium::*;
use graphical::settings::Settings;
use graphical::ui_lib::debayer::Debayer;
use graphical::ui_lib::util::Size::{Percent, Px};
use graphical::ui_lib::util::{AspectRatioContainer, ColorBox, PixelSizeContainer};
use graphical::ui_lib::Drawable;
use graphical::ui_lib::Pos;
use graphical::ui_lib::{Cache, DrawParams};
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
        let cache = &mut BTreeMap::new();

        let mut closed = false;
        while !closed {
            println!("cache size: {}", cache.len());

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
                    self.redraw(image, &gui_settings, cache);
                }
            }

            println!("{} fps", 1000 / now.elapsed().subsec_millis());
        }
    }

    pub fn redraw(&mut self, raw_image: Image, gui_state: &settings::Settings, cache: &mut Cache) {
        let screen_size = self.display.get_framebuffer_dimensions();
        let mut target = self.display.draw();
        target.clear_color(0.0, 0.0, 0.0, 0.0);

        vec![
            &AspectRatioContainer {
                aspect_ratio: 2.0,
                child: &Debayer { raw_image },
            } as &Drawable<Frame>,
            &PixelSizeContainer {
                resolution: (Percent(1.0), Px(80)),
                child: &ColorBox {
                    color: [0.0, 0.0, 0.0, 0.5],
                } as &Drawable<Frame>,
            } as &Drawable<Frame>,
        ].draw(
            &mut DrawParams {
                surface: &mut target,
                facade: &mut self.display,
                cache,
                screen_size,
            },
            Pos::full(),
        );

        target.finish();
    }
}
