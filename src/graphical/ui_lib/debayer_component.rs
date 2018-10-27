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

pub struct Debayer<'a> {
    pub raw_image: &'a Image,
}

impl<'a> Debayer<'a> {
    pub fn debayer(
        raw_image: &Image,
        context: &mut dyn Facade,
        cache: Rc<RefCell<Cache>>,
    ) -> Result<texture::Texture2d, DrawError> {
        flame::start("texture_create");
        let target_texture = texture::Texture2d::empty_with_format(
            context,
            UncompressedFloatFormat::U8U8U8U8,
            MipmapsOption::NoMipmap,
            raw_image.width / 2,
            raw_image.height / 2,
        ).unwrap();
        flame::end("texture_create");

        flame::start("texture_upload");
        let source_texture = texture::Texture2d::new(
            context,
            texture::RawImage2d {
                data: Cow::from(raw_image.data.clone()),
                width: raw_image.width / 2,
                height: raw_image.height / 2,
                format: texture::ClientFormat::U8U8U8U8,
            },
        ).unwrap();
        flame::end("texture_upload");

        flame::start("real");
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
        flame::end("real");

        Ok(target_texture)
    }
}

impl<'a, S> Drawable<S> for Debayer<'a>
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> DrawResult {
        flame::start("debayer");
        let texture = Self::debayer(&self.raw_image, params.facade, params.cache.clone())?;
        flame::end("debayer");
        TextureBox { texture }.draw(params, sp)
    }
}
