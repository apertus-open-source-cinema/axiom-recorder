use self::gl_util::{Vertex, PASSTHROUGH_VERTEX_SHADER_SRC};
use glium::backend::Facade;
use glium::uniforms::Uniforms;
use glium::{index, Blend, DrawError, Program, Surface};
use std::collections::BTreeMap;

pub mod basic_components;
pub mod container_components;
pub mod debayer_component;
mod gl_util;
pub mod layout_components;
pub mod list_components;
pub mod text_components;

// Util type aliases, that allows to pass draw Params easier
pub type Cache = BTreeMap<String, Program>;

pub struct DrawParams<'a, T>
where
    T: Surface + 'a,
{
    pub surface: &'a mut T,
    pub facade: &'a mut dyn Facade,
    pub cache: &'a mut Cache,
    pub screen_size: Vec2<u32>,
}

type DrawResult = Result<(), DrawError>;

/// Util type for representing the "geographical" properties
pub struct Vec2<T> {
    pub x: T,
    pub y: T,
}

impl<T> From<(T, T)> for Vec2<T> {
    fn from(tuple: (T, T)) -> Self {
        Vec2 {
            x: tuple.0,
            y: tuple.1,
        }
    }
}

pub struct SpacialProperties {
    pub start: Vec2<f64>,
    pub size: Vec2<f64>,
}

impl SpacialProperties {
    pub fn full() -> Self {
        SpacialProperties {
            start: Vec2 { x: 0., y: 0. },
            size: Vec2 { x: 1., y: 1. },
        }
    }
}

/// All drawable elements can be rendered with openGL
/// a GUI is a single Drawable, that can contain children
pub trait Drawable<T>
where
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, T>, sp: SpacialProperties) -> DrawResult;
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
    fn draw(&self, params: &mut DrawParams<'_, T>, sp: SpacialProperties) -> DrawResult {
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
                sp.start.x,
                sp.start.y,
                sp.start.x + sp.size.x,
                sp.start.y + sp.size.y,
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
        )
    }
}
