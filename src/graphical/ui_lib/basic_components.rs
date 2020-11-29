use super::*;
use crate::util::error::ResN;
use glium::{
    texture::{RawImage2d, Texture2d},
    uniform,
    uniforms::{Sampler, SamplerWrapFunction},
    Surface,
};

/// Renders a simple colored Box. Useful for semi transparent overlays.
pub struct ColorBox {
    pub color: [f32; 4],
}

impl<S> Drawable<S> for ColorBox
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> ResN {
        ShaderBox {
            fragment_shader: r#"
                #version 330
                out vec4 color;
                uniform vec4 in_color;

                void main(void) {
                    color = in_color;
                }
            "#
            .to_string(),
            uniforms: uniform! {
                in_color: self.color,
            },
        }
        .draw(params, sp)
    }
}

/// renders a simple textured box.
pub struct TextureBox {
    pub texture: Texture2d,
}

impl<S> Drawable<S> for TextureBox
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> ResN {
        ShaderBox {
            fragment_shader: r#"
                #version 330
                uniform sampler2D in_image;
                in vec2 position;
                out vec4 color;

                void main(void) {
                    vec2 pos = position * vec2(1, -1);
                    color = vec4(texture(in_image, pos));
                }
           "#
            .to_string(),
            uniforms: uniform! {
                in_image: &self.texture,
            },
        }
        .draw(params, sp)
    }
}

/// renders a simple textured box with a single color.
pub struct MonoTextureBox {
    pub texture: Texture2d,
    pub color: [f32; 4],
}

impl<S> Drawable<S> for MonoTextureBox
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> ResN {
        let sampler = Sampler::new(&self.texture).wrap_function(SamplerWrapFunction::Clamp);

        ShaderBox {
            fragment_shader: r#"
                #version 330
                uniform sampler2D in_image;
                uniform vec4 in_color;
                in vec2 position;
                out vec4 color;

                void main(void) {
                    ivec2 size = textureSize(in_image, 0);
                    vec2 pos = position * vec2(1, -1) + vec2(0, 1. + (0.5 / size.y));
                    color = in_color * texture(in_image, pos).r;
                }
           "#
            .to_string(),
            uniforms: uniform! {
                in_image: sampler,
                in_color: self.color
            },
        }
        .draw(params, sp)
    }
}


pub struct ImageComponent<'a> {
    pub image: &'a RawImage2d<'a, u8>,
}

impl<'a, S> Drawable<S> for ImageComponent<'a>
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> ResN {
        let image = RawImage2d::from_raw_rgba(
            self.image.data.to_vec(),
            (self.image.width, self.image.height),
        );

        let texture = Texture2d::new(params.facade, image)?;

        TextureBox { texture }.draw(params, sp)
    }
}
