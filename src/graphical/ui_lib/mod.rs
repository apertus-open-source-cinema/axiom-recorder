use self::gl_util::{Vertex, PASSTHROUGH_VERTEX_SHADER_SRC};
use crate::{
    error,
    util::{
        error::{Res, ResN},
        formatting_helpers::code_with_line_numbers,
    },
};
use glium::{backend::Facade, index, uniforms::Uniforms, Blend, Program, Surface};
use itertools::Itertools;
use std::{any::Any, collections::BTreeMap, error};

pub mod basic_components;
pub mod container_components;
mod gl_util;
pub mod histogram_components;
pub mod layout_components;
pub mod list_components;
pub mod text_components;

// Util type aliases, that allows to pass draw Params easier
pub struct Cache(pub BTreeMap<String, Box<dyn Any>>);

impl Cache {
    fn memoize<T, F>(&mut self, key: &String, block: F) -> &T
    where
        F: Fn() -> T,
        T: 'static,
    {
        if !self.0.contains_key(key) {
            self.0.insert(key.clone(), Box::from(block()));
        }
        self.0.get(key).unwrap().as_ref().downcast_ref::<T>().unwrap()
    }

    fn memoize_result<T, F>(&mut self, key: &String, block: F) -> Res<&T>
    where
        F: Fn() -> Res<T>,
        T: 'static,
    {
        if !self.0.contains_key(key) {
            self.0.insert(key.clone(), Box::from(block()?));
        }
        Ok(self.0.get(key).unwrap().as_ref().downcast_ref::<T>().unwrap())
    }
}

pub struct DrawParams<'a, S>
where
    S: Surface + 'a,
{
    pub surface: &'a mut S,
    pub facade: &'a mut dyn Facade,
    pub cache: &'a mut Cache,
    pub screen_size: Vec2<u32>,
}

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
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> ResN;
}

/// Draws a given fragment shader onto a given Box. The heart of all other
/// Drawables
pub struct ShaderBox<U>
where
    U: Uniforms,
{
    pub fragment_shader: String,
    pub uniforms: U,
}

impl<U, S> Drawable<S> for ShaderBox<U>
where
    U: Uniforms,
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> ResN {
        let facade = &params.facade;
        let program = params
            .cache
            .memoize_result(&self.fragment_shader, || {
                let program = Program::from_source(
                    *facade,
                    PASSTHROUGH_VERTEX_SHADER_SRC,
                    self.fragment_shader.as_str(),
                    None,
                )?;
                Ok(program)
            })
            .map_err(|e| {
                error!(
                    "{} \nfragment shader was: \n{}\n",
                    e,
                    code_with_line_numbers(&self.fragment_shader)
                )
            })?;


        let vertices = &Vertex::triangle_strip_surface(
            params.facade,
            (sp.start.x, sp.start.y, sp.start.x + sp.size.x, sp.start.y + sp.size.y),
        );
        let _result = (*params.surface).draw(
            vertices,
            &index::NoIndices(index::PrimitiveType::TriangleStrip),
            program,
            &self.uniforms,
            &glium::DrawParameters { blend: Blend::alpha_blending(), ..Default::default() },
        )?;
        Ok(())
    }
}
