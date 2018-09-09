extern crate glium;

use glium::backend::Facade;
use glium::index;
use glium::uniforms::Uniforms;
use glium::Frame;
use glium::Program;
use glium::Surface;
use graphical::gl_util::{Vertex, PASSTHROUGH_VERTEX_SHADER_SRC};
use std::collections::BTreeMap;

pub mod util;
pub mod debayer;

/// Util type alias, that allows to pass draw Params easier
type DrawParams<'a> = (
    &'a mut Frame,
    &'a mut Facade,
    &'a mut BTreeMap<String, Program>,
);

/// Util type for representing the "geographical" properties
pub struct Pos {
    pub start: (f32, f32),
    pub size: (f32, f32),
}

/// All drawable elements can be rendered with openGL
/// a GUI is a single Drawable, that can contain children
pub trait Drawable {
    fn draw(&self, params: &mut DrawParams, pos: Pos);
}

/// Draws a given fragment shader onto a given Box. The heart of all other Drawables
pub struct ShaderBox<U>
where
    U: Uniforms,
{
    fragment_shader: String,
    uniforms: U,
}

impl<U> Drawable for ShaderBox<U>
where
    U: Uniforms,
{
    fn draw(&self, params: &mut DrawParams, pos: Pos) {
        let (frame, facade, cache) = params;

        if !cache.contains_key(self.fragment_shader.as_str()) {
            let fragment_shader = self.fragment_shader.clone();
            let program = Program::from_source(
                *facade,
                PASSTHROUGH_VERTEX_SHADER_SRC,
                self.fragment_shader.as_str(),
                None,
            ).unwrap();
            cache.insert(fragment_shader, program);
        }

        let program = cache.get(self.fragment_shader.as_str()).unwrap();

        let vertices = &Vertex::triangle_strip_surface(
            *facade,
            (pos.start.0, pos.start.1, pos.start.0 + pos.size.0, pos.start.1 + pos.size.1),
        );
        (*frame).draw(
            vertices,
            &index::NoIndices(index::PrimitiveType::TriangleStrip),
            &program,
            &self.uniforms,
            &Default::default(),
        );
    }
}
