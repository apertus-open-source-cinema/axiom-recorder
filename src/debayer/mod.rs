use crate::{
    error,
    graphical::ui_lib::{Cache, DrawParams, Drawable, ShaderBox, SpatialProperties, Vec2},
    util::error::Res,
};
use glium::{
    backend::{glutin::headless::Headless, Facade},
    texture::{self, pixel_buffer::PixelBuffer, MipmapsOption, Texture2d, UncompressedFloatFormat},
    Surface,
};
use std::{
    borrow::Cow,
    collections::btree_map::BTreeMap,
    error,
    result::Result::Ok,
    time::Instant,
};

use crate::{
    debayer::shader_builder::{F32OptionMap, F32OptionMapTextureUniforms, ShaderBuilder},
    util::image::Image,
};
use glium::texture::RawImage2d;
use glutin::dpi::PhysicalSize;


pub mod shader_builder;

pub trait Debayer {
    //    fn debayer(&self, debayerer: &mut Debayerer) -> Result<RawImage2d<u8>,
    // Box<dyn error::Error>>;
    fn debayer_drawable(
        &self,
        debayerer: &mut Debayerer,
        facade: &mut dyn Facade,
    ) -> Res<Texture2d>;
}

impl Debayer for Image {
    fn debayer_drawable(
        &self,
        debayerer: &mut Debayerer,
        facade: &mut dyn Facade,
    ) -> Res<Texture2d> {
        let fragment_shader = debayerer.get_code();
        let target_size = debayerer.get_size();

        (*debayerer).buffer_index = (debayerer.buffer_index + 1) % 2;
        let next_index = (debayerer.buffer_index + 1) % 2;

        // let uniforms = debayerer.get_uniforms(&debayerer.source_texture);

        debayerer.source_texture.main_level().raw_upload_from_pixel_buffer(
            debayerer.source_buffers[debayerer.buffer_index as usize].as_slice(),
            0..self.width,
            0..self.height,
            0..1,
        );

        if self.data.len() != debayerer.source_buffers[0].get_size() {
            println!(
                "something is wrong : self.data.len() != debayerer.source_buffers[0].get_size()"
            );
            println!("{} != {}", self.data.len(), debayerer.source_buffers[0].get_size())
        } else {
            debayerer.source_buffers[next_index as usize].write(&self.data);
        }

        // println!("self.data.len() {}", self.data.len());


        let target_texture: Texture2d = Texture2d::empty_with_format(
            facade,
            UncompressedFloatFormat::U8U8U8U8,
            MipmapsOption::NoMipmap,
            target_size.0,
            target_size.1,
        )?;

        ShaderBox {
            fragment_shader,
            uniforms: F32OptionMapTextureUniforms((
                debayerer.uniforms.clone(),
                &debayerer.source_texture,
            )),
        }
        .draw(
            &mut DrawParams {
                surface: &mut target_texture.as_surface(),
                facade,
                cache: debayerer.cache.as_mut(),
                // screen_size: Vec2 { x: self.width, y: self.height },
                screen_size: Vec2 { x: target_size.0, y: target_size.1 },
            },
            SpatialProperties::full(),
        )?;

        Ok(target_texture)
    }

    /*
    fn debayer(&self, debayerer: &mut Debayerer) -> Res<RawImage2d<u8>> {
        let texture_data_sink = self.debayer_drawable(debayerer, None)?.read();

        Ok(texture_data_sink)
    }
    */
}

pub struct Debayerer {
    pub source_texture: Texture2d,
    pub target_texture: Texture2d,
    pub source_buffers: [PixelBuffer<u8>; 2],
    pub buffer_index: u8,
    pub cache: Box<Cache>,
    code: String,
    size: (u32, u32),
    uniforms: F32OptionMap,
}

impl Debayerer {
    pub fn new(debayer_options: &str, size: (u32, u32), context: &mut dyn Facade) -> Res<Self> {
        let cache = Cache(BTreeMap::new());

        let shader_builder = ShaderBuilder::from_descr_str(debayer_options)?;
        let implications = shader_builder.get_implications();
        let target_size = match implications.get("resolution_div") {
            Some(div) => {
                let divider: u32 = div
                    .as_ref()
                    .ok_or(error!("implication resolution_div needs a parameter!"))?
                    .parse()?;
                (size.0 / divider, size.1 / divider)
            }
            None => size,
        };

        let source_texture = Texture2d::empty_with_format(
            context,
            UncompressedFloatFormat::U8,
            MipmapsOption::NoMipmap,
            size.0,
            size.1,
        )?;

        let target_texture: Texture2d = Texture2d::empty_with_format(
            context,
            UncompressedFloatFormat::U8U8U8U8,
            MipmapsOption::NoMipmap,
            target_size.0,
            target_size.1,
        )?;

        println!("size {:?}", size);
        let buffer_capacity = (size.0 * size.1) as usize;

        let source_buffers = [
            PixelBuffer::new_empty(context, buffer_capacity),
            PixelBuffer::new_empty(context, buffer_capacity),
        ];

        println!("target_size {:?}", target_size);

        Ok(Self {
            cache: Box::new(cache),
            code: shader_builder.get_code(),
            size: target_size,
            uniforms: shader_builder.get_uniforms(),
            source_texture,
            target_texture,
            source_buffers,
            buffer_index: 0,
        })
    }

    fn get_code(&self) -> String { self.code.clone() }

    pub fn get_size(&self) -> (u32, u32) { self.size.clone() }

    fn get_uniforms<'a>(&'a self, texture: &'a Texture2d) -> F32OptionMapTextureUniforms {
        F32OptionMapTextureUniforms((self.uniforms.clone(), texture))
    }
}
