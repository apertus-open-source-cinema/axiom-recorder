extern crate glium;
use glium::*;
use glium::glutin::{WindowBuilder, ContextBuilder, EventsLoop};
use video_io::Image;
use glium::texture::texture2d::Texture2d;
use glium::backend::Facade;
use std::borrow::Cow;

mod settings;
mod gl_util;


/// Manage the rendering process and orchestrate the rendering passes
pub struct Manager {
    display: Display,
}

impl Manager {
    pub fn new() -> Manager {
        let mut events_loop = EventsLoop::new();
        let window = WindowBuilder::new();
        let context = ContextBuilder::new();
        let display = Display::new(window, context, &events_loop).unwrap();

        Manager {display}
    }

    pub fn draw(&self, debayered_image: texture::Texture2d, gui_state: &settings::Settings) {
        let mut target = self.display.draw();

        let program = Program::from_source(
            &self.display,
            gl_util::PASSTHROUGH_VERTEX_SHADER_SRC,
            gl_util::PASSTHROUGH_FRAGMENT_SHADER_SRC,
            None
        ).unwrap();


        target.draw(
            &gl_util::Vertex::create_triangle_strip_vertex_buffer(&self.display),
            &index::NoIndices(index::PrimitiveType::TriangleStrip),
            &program,
            &uniforms::EmptyUniforms,
            &Default::default(),
        ).unwrap();
        target.finish();
    }

    pub fn debayer(raw_image: Image, context: &Facade) -> Texture2d {
        let target_texture = Texture2d::empty(
            context,
            raw_image.width,
            raw_image.height
        ).unwrap();

        let source_texture = Texture2d::new(
            context,
            texture::RawImage2d {data: Cow::from(raw_image.data), width: raw_image.width, height: raw_image.height, format: texture::ClientFormat::U8}
        ).unwrap();

        let program = Program::from_source(
            context,
            gl_util::PASSTHROUGH_VERTEX_SHADER_SRC,
            include_str!("debayer.frag"),
            None
        ).unwrap();

        target_texture.as_surface().draw(
            &gl_util::Vertex::create_triangle_strip_vertex_buffer(context),
            &index::NoIndices(index::PrimitiveType::TriangleStrip),
            &program,
            &uniforms::EmptyUniforms,
            &Default::default()
        );

        target_texture
    }

    pub fn redraw(&self, raw_image: Image, gui_state: &settings::Settings) {
        // Redraws the whole window by invoking a debayer and the second rendering pass
        let debayered = Manager::debayer(raw_image, &self.display);
        self.draw(debayered, gui_state)
    }
}
