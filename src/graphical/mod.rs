extern crate glium;

use glium::*;
use glium::glutin::{WindowBuilder, ContextBuilder, EventsLoop};
use video_io::Image;
use glium::texture::texture2d::Texture2d;
use glium::backend::Facade;
use std::borrow::Cow;
use std::thread;
use bus::BusReader;
use graphical::settings::Settings;
use std::time::Duration;
use glium::texture::UncompressedFloatFormat;
use glium::texture::MipmapsOption;

mod settings;
mod gl_util;


/// Manage the rendering process and orchestrate the rendering passes
pub struct Manager {
    display: Display,
    raw_image_source: BusReader<Image>,
    event_loop: EventsLoop,
}

impl Manager {
    pub fn new(raw_image_source: BusReader<Image>) -> Self {
        let mut event_loop = EventsLoop::new();
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
                recording_format: settings::RecordingFormat::rawN,
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

    pub fn assemble(&self, debayered_image: texture::Texture2d, gui_state: &settings::Settings) {
        let mut target = self.display.draw();

        let program = Program::from_source(
            &self.display,
            gl_util::PASSTHROUGH_VERTEX_SHADER_SRC,
            include_str!("scale.frag"),
            None,
        ).unwrap();

        target.draw(
            &gl_util::Vertex::create_triangle_strip_vertex_buffer(&self.display),
            &index::NoIndices(index::PrimitiveType::TriangleStrip),
            &program,
            &uniform! {in_image: &debayered_image},
            &Default::default(),
        ).unwrap();

        target.finish().unwrap();
    }

    pub fn debayer(raw_image: Image, context: &Facade) -> Texture2d {
        let target_texture = Texture2d::empty_with_format(
            context,
            UncompressedFloatFormat::U8U8U8U8,
            MipmapsOption::NoMipmap,
            raw_image.width,
            raw_image.height,
        ).unwrap();

        let source_texture = Texture2d::new(
            context,
            texture::RawImage2d {
                data: Cow::from(raw_image.data),
                width: raw_image.width, height: raw_image.height,
                format: texture::ClientFormat::U8
            },
        ).unwrap();

        let program = Program::from_source(
            context,
            gl_util::PASSTHROUGH_VERTEX_SHADER_SRC,
            include_str!("debayer.frag"),
            None,
        ).unwrap();

        target_texture.as_surface().draw(
            &gl_util::Vertex::create_triangle_strip_vertex_buffer(context),
            &index::NoIndices(index::PrimitiveType::TriangleStrip),
            &program,
            &uniform! {raw_image: &source_texture},
            &Default::default(),
        );

        target_texture
    }

    pub fn redraw(&self, raw_image: Image, gui_state: &settings::Settings) {
        // Redraws the whole window by invoking a debayer and the second rendering pass
        let debayered = Manager::debayer(raw_image, &self.display);
        self.assemble(debayered, gui_state)
    }
}
