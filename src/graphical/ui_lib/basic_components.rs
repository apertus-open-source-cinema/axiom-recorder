use super::*;
use glium::{texture, uniform, Surface};

/// Renders a simple colored Box. Useful for semi transparent overlays.
pub struct ColorBox {
    pub color: [f32; 4],
}

impl<S> Drawable<S> for ColorBox
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> DrawResult {
        ShaderBox {
            fragment_shader: r#"
                #version 450
                out vec4 color;
                uniform vec4 in_color;

                void main(void) {
                    color = in_color;
                }
            "#.to_string(),
            uniforms: uniform! {
                in_color: self.color,
            },
        }.draw(params, sp)
    }
}

/// renders a simple textured box.
pub struct TextureBox {
    pub texture: texture::Texture2d,
}

impl<S> Drawable<S> for TextureBox
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> DrawResult {
        ShaderBox {
            fragment_shader: r#"
                #version 450
                uniform sampler2D in_image;
                in vec2 position;
                out vec4 color;

                void main(void) {
                    vec2 pos = position * vec2(1, -1);
                    color = vec4(texture(in_image, pos));
                }
           "#.to_string(),
            uniforms: uniform! {
                in_image: &self.texture,
            },
        }.draw(params, sp)
    }
}

/// renders a simple textured box with a single color.
pub struct MonoTextureBox<'a> {
    pub texture: &'a texture::Texture2d,
    pub color: [f32; 4],
}

impl<'a, S> Drawable<S> for MonoTextureBox<'a>
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> DrawResult {
        ShaderBox {
            fragment_shader: r#"
                #version 450
                uniform sampler2D in_image;
                uniform vec4 in_color;
                in vec2 position;
                out vec4 color;

                void main(void) {
                    vec2 pos = position * vec2(1, -1);
                    color = texture(in_image, pos).r * in_color;
                }
           "#.to_string(),
            uniforms: uniform! {
                in_image: self.texture,
                in_color: self.color
            },
        }.draw(params, sp)
    }
}
