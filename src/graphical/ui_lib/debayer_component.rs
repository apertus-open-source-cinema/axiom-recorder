use self::basic_components::TextureBox;
use super::*;
use crate::video_io::Image;
use glium::{
    backend::Facade,
    texture::{self, MipmapsOption, UncompressedFloatFormat},
    uniform,
    DrawError,
    Surface,
};
use std::{borrow::Cow, result::Result::Ok};

pub struct Debayer {
    pub raw_image: Image,
}

impl Debayer {
    pub fn debayer(
        raw_image: &Image,
        context: &mut dyn Facade,
        cache: &mut Cache,
    ) -> Result<texture::Texture2d, DrawError> {
        let target_texture = texture::Texture2d::empty_with_format(
            context,
            UncompressedFloatFormat::U8U8U8U8,
            MipmapsOption::NoMipmap,
            raw_image.width,
            raw_image.height,
        ).unwrap();

        let source_texture = texture::Texture2d::new(
            context,
            texture::RawImage2d {
                data: Cow::from(raw_image.data.clone()),
                width: raw_image.width,
                height: raw_image.height,
                format: texture::ClientFormat::U8,
            },
        ).unwrap();

        ShaderBox {
            fragment_shader: include_str!("debayer.frag").to_string(),
            uniforms: uniform! {raw_image: &source_texture},
        }.draw(
            &mut DrawParams {
                surface: &mut target_texture.as_surface(),
                facade: context,
                cache,
                screen_size: Vec2 { x: raw_image.width, y: raw_image.height },
            },
            SpatialProperties::full(),
        )?;

        Ok(target_texture)
    }
}

impl<S> Drawable<S> for Debayer
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> DrawResult {
        let texture = Self::debayer(&self.raw_image, params.facade, params.cache)?;
        TextureBox { texture }.draw(params, sp)
    }
}
