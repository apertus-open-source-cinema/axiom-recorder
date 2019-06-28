use crate::{
    error,
    graphical::ui_lib::{Cache, DrawParams, Drawable, ShaderBox, SpatialProperties, Vec2},
    util::error::Res,
};
use glium::{
    backend::glutin::headless::Headless,
    texture::{self, MipmapsOption, Texture2d, UncompressedFloatFormat},
};
use glutin::{ContextBuilder, EventsLoop};
use std::{borrow::Cow, collections::btree_map::BTreeMap, error, result::Result::Ok};

use crate::{
    debayer::shader_builder::{F32OptionMap, F32OptionMapTextureUniforms, ShaderBuilder},
    util::image::Image,
};
use glium::texture::RawImage2d;
use glutin::dpi::PhysicalSize;


pub mod shader_builder;

pub trait Debayer {
    fn debayer(&self, debayerer: &mut Debayerer) -> Result<RawImage2d<u8>, Box<dyn error::Error>>;
}

impl Debayer for Image {
    fn debayer(&self, debayerer: &mut Debayerer) -> Res<RawImage2d<u8>> {
        let source_texture = Box::new(Texture2d::new(
            debayerer.facade.as_mut(),
            texture::RawImage2d {
                data: Cow::from(self.data.clone()),
                width: self.width,
                height: self.height,
                format: texture::ClientFormat::U8,
            },
        )?);

        let target_size = debayerer.get_size();
        let target_texture: Texture2d = Texture2d::empty_with_format(
            debayerer.facade.as_mut(),
            UncompressedFloatFormat::U8U8U8U8,
            MipmapsOption::NoMipmap,
            target_size.0,
            target_size.1,
        )?;

        ShaderBox {
            fragment_shader: debayerer.get_code(),
            uniforms: debayerer.get_uniforms(source_texture),
        }
        .draw(
            &mut DrawParams {
                surface: &mut target_texture.as_surface(),
                facade: debayerer.facade.as_mut(),
                cache: debayerer.cache.as_mut(),
                screen_size: Vec2 { x: self.width, y: self.height },
            },
            SpatialProperties::full(),
        )?;


        let texture_data_sink = target_texture.read();

        Ok(texture_data_sink)
    }
}

pub struct Debayerer {
    pub facade: Box<Headless>,
    pub cache: Box<Cache>,
    code: String,
    size: (u32, u32),
    uniforms: F32OptionMap,
}

impl Debayerer {
    pub fn new(debayer_options: &str, size: (u32, u32)) -> Res<Self> {
        let context = ContextBuilder::new()
            .build_headless(&EventsLoop::new(), PhysicalSize::new(1.0, 1.0))?;
        let facade = Headless::new(context)?;
        let cache = Cache(BTreeMap::new());

        let shader_builder = ShaderBuilder::from_descr_str(debayer_options)?;
        let implications = shader_builder.get_implications();
        let size = match implications.get("resolution_div") {
            Some(div) => {
                let divider: u32 = div
                    .as_ref()
                    .ok_or(error!("implication resolution_div needs a parameter!"))?
                    .parse()?;
                (size.0 / divider, size.1 / divider)
            }
            None => size,
        };

        Ok(Self {
            facade: Box::new(facade),
            cache: Box::new(cache),
            code: shader_builder.get_code(),
            size,
            uniforms: shader_builder.get_uniforms(),
        })
    }

    fn get_code(&self) -> String { self.code.clone() }

    pub fn get_size(&self) -> (u32, u32) { self.size.clone() }

    fn get_uniforms(&self, texture: Box<Texture2d>) -> F32OptionMapTextureUniforms {
        F32OptionMapTextureUniforms((self.uniforms.clone(), texture))
    }
}
