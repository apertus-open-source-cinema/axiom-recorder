use crate::{
    graphical::ui_lib::{Cache, DrawParams, Drawable, ShaderBox, SpatialProperties, Vec2},
    util::error::Res,
    video_io::Image,
};
use glium::{
    backend::glutin::headless::Headless,
    texture::{self, MipmapsOption, Texture2d, UncompressedFloatFormat},
    uniform,
};
use glutin::{ContextBuilder, EventsLoop};
use std::{borrow::Cow, collections::btree_map::BTreeMap, error, result::Result::Ok};

use crate::debayer::shader_builder::ShaderBuilder;
use glium::{
    texture::RawImage2d,
    uniforms::{Uniforms, UniformsStorage},
};
use glutin::dpi::PhysicalSize;
use include_dir::{Dir, *};
use regex::Regex;
use std::{collections::HashMap, hash::Hash, io::Error, panic::set_hook};

mod shader_builder;

pub trait Debayer {
    fn debayer(&self, debayer_options: &str) -> Result<RawImage2d<u8>, Box<dyn error::Error>>;
}

impl Debayer for Image {
    fn debayer(&self, debayer_options: &str) -> Res<RawImage2d<u8>> {
        let context = ContextBuilder::new()
            .build_headless(&EventsLoop::new(), PhysicalSize::new(1.0, 1.0))?;
        let facade = &mut Headless::new(context)?;
        let cache = &mut Cache(BTreeMap::new());

        let target_texture: Texture2d = Texture2d::empty_with_format(
            facade,
            UncompressedFloatFormat::U8U8U8U8,
            MipmapsOption::NoMipmap,
            self.width,
            self.height,
        )
        .unwrap();

        let source_texture = Texture2d::new(
            facade,
            texture::RawImage2d {
                data: Cow::from(self.data.clone()),
                width: self.width,
                height: self.height,
                format: texture::ClientFormat::U8,
            },
        )
        .unwrap();

        let shader_builder = ShaderBuilder::from_descr_str(debayer_options)?;

        ShaderBox {
            fragment_shader: shader_builder.get_code(),
            uniforms: shader_builder.get_uniforms(),
        }
        .draw(
            &mut DrawParams {
                surface: &mut target_texture.as_surface(),
                facade,
                cache,
                screen_size: Vec2 { x: self.width, y: self.height },
            },
            SpatialProperties::full(),
        )?;

        let texture_data_sink = target_texture.read();

        Ok(texture_data_sink)
    }
}
