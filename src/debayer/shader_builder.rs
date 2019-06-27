use crate::{
    graphical::ui_lib::{Cache, DrawParams, Drawable, ShaderBox, SpatialProperties, Vec2},
    throw,
    util::error::{Error, Res},
    video_io::Image,
};
use glium::{
    backend::glutin::headless::Headless,
    texture::{self, MipmapsOption, Texture2d, UncompressedFloatFormat},
    uniform,
};
use glutin::{ContextBuilder, EventsLoop};
use std::{borrow::Cow, collections::btree_map::BTreeMap, error, result::Result::Ok};

use glium::{
    texture::RawImage2d,
    uniforms::{AsUniformValue, EmptyUniforms, UniformValue, Uniforms, UniformsStorage},
};
use glutin::dpi::PhysicalSize;
use include_dir::{Dir, *};
use regex::Regex;
use std::{collections::HashMap, hash::Hash, panic::set_hook};


// this is only a newtype because rusts prohibition of implementing foreign
// traits for foreign Types sucks
#[derive(Clone)]
pub struct F32Uniforms(pub HashMap<String, Option<f32>>);
type Implications = HashMap<String, Option<String>>;

impl Uniforms for F32Uniforms {
    fn visit_values<'a, F: FnMut(&str, UniformValue<'a>)>(&'a self, mut callback: F) {
        for (k, v) in &self.0 {
            callback(k, UniformValue::Float(v.unwrap()));
        }
    }
}

// statically pull some shaders into the binary
static SHADERS: Dir = include_dir!("src/debayer/shader");

pub struct ShaderBuilder {
    shader_parts: Vec<ShaderBuilderPart>,
}

impl ShaderBuilder {
    pub fn from_descr_str(descr_str: &str) -> Res<Self> {
        let re = Regex::new("(\\.?/?[a-z_]*)\\((.*?)\\)").unwrap();
        let shader_parts = Vec::new();
        for cap in re.captures_iter(descr_str.as_ref()) {
            let shader_part_name = cap.get(1).unwrap().as_str();
            let shader_params = format!("{}.glsl", cap.get(2).unwrap().as_str());

            let shader_code = if shader_part_name.contains("/") {
                // Shader should be read from fs
                unimplemented!()
            } else {
                // A builtin Shader should be used
                SHADERS
                    .get_file(shader_part_name)
                    .ok_or(Error::new(format!(
                        "shader '{}' is not buildin to the binary. Did you mean './{} ?'",
                        shader_part_name, shader_part_name
                    )))?
                    .contents_utf8()
                    .unwrap()
            };
        }

        Ok(Self { shader_parts })
    }

    pub fn get_available() -> HashMap<String, (F32Uniforms, Implications)> { unimplemented!() }

    pub fn get_implications(&self) -> Implications {
        let mut to_return = HashMap::new();
        for part in &self.shader_parts {
            for (k, v) in part.get_implications() {
                to_return.insert(k, v);
            }
        }
        to_return
    }

    pub fn get_uniforms(&self) -> F32Uniforms {
        let mut to_return = HashMap::new();
        for part in &self.shader_parts {
            for (k, v) in part.get_uniforms().0 {
                to_return.insert(k, v);
            }
        }
        F32Uniforms(to_return)
    }

    pub fn get_code(&self) -> String {
        let mut to_return = String::new();
        for part in &self.shader_parts {
            to_return += &part.get_code();
        }
        String::from(to_return)
    }
}

pub struct ShaderBuilderPart {
    code: String,
    uniforms: F32Uniforms,
}

impl ShaderBuilderPart {
    fn new(code: String, non_default_uniforms: F32Uniforms) -> Res<Self> {
        let re =
            Regex::new("uniform\\s+float\\s+(\\w+)\\s*;\\s*//\\s*=\\s*(\\d*\\.?\\d*)").unwrap();
        let mut uniforms = F32Uniforms(HashMap::new()).0;

        let mut taken = 0;
        for cap in re.captures_iter(code.as_str()) {
            let uniform_name = cap.get(1).unwrap().as_str();
            let value = non_default_uniforms.0.get(uniform_name).unwrap_or(
                uniforms.get(uniform_name).ok_or(Error::new(format!(
                    "uniform {} is has no default and is not set.",
                    uniform_name
                )))?,
            );
            taken += 1;

            uniforms.insert(
                String::from(uniform_name),
                cap.get(2).map(|v| v.as_str().parse().unwrap()),
            );
        }

        if taken != uniforms.len() {
            throw!("some uniform values were not consumed by that shader. maybe you set nonexistent uniforms?")
        }

        Ok(ShaderBuilderPart { code, uniforms: F32Uniforms(uniforms) })
    }

    fn new_with_str_params(code: String, params: String) -> Res<Self> {
        let re = Regex::new("(\\w+):\\s*(\\d*\\.?\\d*)").unwrap();
        let mut non_default_uniforms: HashMap<String, Option<f32>> = HashMap::new();
        for cap in re.captures_iter(params.as_str()) {
            non_default_uniforms.insert(
                String::from(cap.get(1).unwrap().as_str()),
                cap.get(2).map(|v| v.as_str().parse().unwrap()),
            );
        }

        Self::new(code, F32Uniforms(non_default_uniforms))
    }

    fn get_uniforms(&self) -> F32Uniforms { self.uniforms.clone() }

    fn get_implications(&self) -> Implications {
        let re = Regex::new("! (.*)(\\s?=\\s?(.*))").unwrap();
        let mut result = HashMap::new();
        for cap in re.captures_iter(&self.code) {
            result.insert(
                String::from(cap.get(1).unwrap().as_str()),
                cap.get(3).map(|x| String::from(x.as_str())),
            );
        }

        result
    }

    fn get_code(&self) -> String { self.code.clone() }
}
