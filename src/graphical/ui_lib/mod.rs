extern crate glium;

use glium::backend::Facade;
use glium::index;
use glium::uniforms::Uniforms;
use glium::Blend;
use glium::Frame;
use glium::Program;
use glium::Surface;
use graphical::gl_util::{Vertex, PASSTHROUGH_VERTEX_SHADER_SRC};
use std::collections::BTreeMap;

pub mod debayer;
pub mod util;

// Util type aliases, that allows to pass draw Params easier
pub type Cache = BTreeMap<String, Program>;
pub struct DrawParams<'a, T>
where
    T: Surface + 'a,
{
    pub surface: &'a mut T,
    pub facade: &'a mut Facade,
    pub cache: &'a mut Cache,
    pub screen_size: (u32, u32),
}

/// Util type for representing the "geographical" properties
pub struct Pos {
    pub start: (f32, f32),
    pub size: (f32, f32),
}

impl Pos {
    pub fn full() -> Self {
        Pos {
            start: (0., 0.),
            size: (1., 1.),
        }
    }
}

/// All drawable elements can be rendered with openGL
/// a GUI is a single Drawable, that can contain children
pub trait Drawable<T>
where
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos);
}

/// Draws a given fragment shader onto a given Box. The heart of all other Drawables
pub struct ShaderBox<U>
where
    U: Uniforms,
{
    fragment_shader: String,
    uniforms: U,
}

impl<U, T> Drawable<T> for ShaderBox<U>
where
    U: Uniforms,
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) {
        if !params.cache.contains_key(self.fragment_shader.as_str()) {
            let fragment_shader = self.fragment_shader.clone();
            let program = Program::from_source(
                params.facade,
                PASSTHROUGH_VERTEX_SHADER_SRC,
                self.fragment_shader.as_str(),
                None,
            ).unwrap();
            params.cache.insert(fragment_shader, program);
        }

        let program = params.cache.get(self.fragment_shader.as_str()).unwrap();

        let vertices = &Vertex::triangle_strip_surface(
            params.facade,
            (
                pos.start.0,
                pos.start.1,
                pos.start.0 + pos.size.0,
                pos.start.1 + pos.size.1,
            ),
        );
        (*params.surface).draw(
            vertices,
            &index::NoIndices(index::PrimitiveType::TriangleStrip),
            &program,
            &self.uniforms,
            &glium::DrawParameters {
                blend: Blend::alpha_blending(),
                ..Default::default()
            },
        );
    }
}
