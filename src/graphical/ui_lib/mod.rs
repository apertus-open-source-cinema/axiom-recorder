use self::gl_util::{Vertex, PASSTHROUGH_VERTEX_SHADER_SRC};
use glium::{backend::Facade, index, uniforms::Uniforms, Blend, DrawError, Program, Surface};
use std::collections::BTreeMap;

pub mod basic_components;
pub mod container_components;
pub mod debayer_component;
mod gl_util;
pub mod histogram_components;
pub mod layout_components;
pub mod list_components;
pub mod text_components;

// Util type aliases, that allows to pass draw Params easier
pub type Cache = BTreeMap<String, Program>;

pub struct DrawParams<'a, S>
where
    S: Surface + 'a,
{
    pub surface: &'a mut S,
    pub facade: &'a mut dyn Facade,
    pub cache: &'a mut Cache,
    pub screen_size: Vec2<u32>,
}

type DrawResult = Result<(), DrawError>;

/// Util type for representing the "geographical" properties
#[derive(Debug, Clone)]
pub struct Vec2<T> {
    pub x: T,
    pub y: T,
}

impl<T> Vec2<T>
where
    T: From<u32>,
{
    pub fn zero() -> Self { Vec2 { x: T::from(0), y: T::from(0) } }
    pub fn one() -> Self { Vec2 { x: T::from(1), y: T::from(1) } }
}

impl<T> From<(T, T)> for Vec2<T> {
    fn from(tuple: (T, T)) -> Self { Vec2 { x: tuple.0, y: tuple.1 } }
}

#[derive(Debug, Clone)]
pub struct SpatialProperties {
    pub start: Vec2<f64>,
    pub size: Vec2<f64>,
}

impl SpatialProperties {
    pub fn full() -> Self { SpatialProperties { start: Vec2::zero(), size: Vec2::one() } }
}

/// All drawable elements can be rendered with openGL
/// a GUI is a single Drawable, that can contain children
pub trait Drawable<S>
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> DrawResult;
}

/// Draws a given fragment shader onto a given Box. The heart of all other
/// Drawables
pub struct ShaderBox<U>
where
    U: Uniforms,
{
    fragment_shader: String,
    uniforms: U,
}

impl<U, S> Drawable<S> for ShaderBox<U>
where
    U: Uniforms,
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> DrawResult {
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
            (sp.start.x, sp.start.y, sp.start.x + sp.size.x, sp.start.y + sp.size.y),
        );
        (*params.surface).draw(
            vertices,
            &index::NoIndices(index::PrimitiveType::TriangleStrip),
            &program,
            &self.uniforms,
            &glium::DrawParameters { blend: Blend::alpha_blending(), ..Default::default() },
        )
    }
}
