use self::gl_util::{Vertex, PASSTHROUGH_VERTEX_SHADER_SRC};
use crate::{
    error,
    util::{
        error::{Res, ResN},
        formatting_helpers::code_with_line_numbers,
    },
};
use glium::{backend::Facade, index, uniforms::Uniforms, Blend, Program, Surface};

use std::{any::Any, collections::BTreeMap};
use threadpool::ThreadPool;
use std::sync::{Mutex, Arc};

pub mod basic_components;
pub mod container_components;
mod gl_util;
pub mod histogram_components;
pub mod layout_components;
pub mod list_components;
pub mod text_components;

// Util type aliases, that allows to pass draw Params easier
pub struct Cache {
    map: BTreeMap<String, Box<dyn Any>>,
}

impl Cache {
    pub fn new() -> Self {
        Cache { map: BTreeMap::new() }
    }

    pub fn memoize<T, F>(&mut self, key: String, block: F) -> &T
    where
        F: Fn() -> T,
        T: 'static,
    {
        if !self.map.contains_key(&key.clone()) {
            self.map.insert(key.clone(), Box::from(block()));
        }
        self.map.get(&key.clone()).unwrap().as_ref().downcast_ref::<T>().unwrap()
    }

    pub fn memoize_result<T, F>(&mut self, key: String, block: F) -> Res<&T>
    where
        F: Fn() -> Res<T>,
        T: 'static,
    {
        if !self.map.contains_key(&key) {
            self.map.insert(key.clone(), Box::from(block()?));
        }
        Ok(self.map.get(&key).unwrap().as_ref().downcast_ref::<T>().unwrap())
    }
}

pub struct Deferrer {
    map: Arc<Mutex<Box<BTreeMap<String, Arc<Mutex<Box<dyn Any + Send>>>>>>>,
    threadpool: ThreadPool,
}

impl Deferrer {
    pub fn new() -> Self {
        Self { map: Arc::new(Mutex::new(Box::new(BTreeMap::new()))), threadpool: ThreadPool::new(num_cpus::get() - 1) }
    }

    pub fn deferred_do<T, F>(&mut self, key: String, block: F, default: T) -> &T
    where
        F: 'static + Fn() -> T + Send,
        T: 'static + Send
    {
        let mut map = &self.map.clone();
        let key_for_thread = key.clone();

        self.threadpool.execute(move || {
            map.lock().unwrap().insert(key_for_thread, Arc::new(Mutex::new(Box::from(block()))));
        });

        match self.map.lock().unwrap().get(&key) {
            Some(v) => v.as_ref().lock().unwrap().downcast_ref::<T>().unwrap().clone(),
            None => &default
        }
    }
}

pub struct DrawParams<'a, S>
where
    S: Surface + 'a,
{
    pub surface: &'a mut S,
    pub facade: &'a mut dyn Facade,
    pub cache: &'a mut Cache,
    pub deferrer: Option<&'a mut Deferrer>,
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
            .memoize_result(self.fragment_shader.clone(), || {
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
        (*params.surface).draw(
            vertices,
            &index::NoIndices(index::PrimitiveType::TriangleStrip),
            program,
            &self.uniforms,
            &glium::DrawParameters { blend: Blend::alpha_blending(), ..Default::default() },
        )?;
        Ok(())
    }
}
