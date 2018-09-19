use glium::backend::Facade;
use glium::texture;
use glium::texture::MipmapsOption;
use glium::texture::UncompressedFloatFormat;
use glium::DrawError;
use glium::Surface;
use graphical::ui_lib::*;
use std::borrow::Cow;
use std::result::Result::Ok;
use video_io::Image;

pub struct Debayer {
    pub raw_image: Image,
}

impl Debayer {
    pub fn debayer(
        raw_image: &Image,
        context: &mut Facade,
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
                screen_size: Vec2 {
                    x: raw_image.width,
                    y: raw_image.height,
                },
            },
            SpacialProperties::full(),
        )?;

        Ok(target_texture)
    }
}

impl<T> Drawable<T> for Debayer
where
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<T>, sp: SpacialProperties) -> DrawResult {
        let texture = Self::debayer(&self.raw_image, params.facade, params.cache)?;

        ShaderBox {
            fragment_shader: r#"
                #version 450
                uniform sampler2D in_image;
                in vec2 position;
                out vec4 color;

                void main(void) {
                    ivec2 size = textureSize(in_image, 0);
                    ivec2 pos = ivec2(size * position);
                    pos.y = size.y - pos.y;
                    color = vec4(texelFetch(in_image, pos, 0));
                }
           "#.to_string(),
            uniforms: uniform! {
                in_image: &texture,
            },
        }.draw(params, sp)
    }
}
