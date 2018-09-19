use crate::graphical::ui_lib::*;
use glium::texture;
use glium::Surface;

/// Renders a simple colored Box. Useful for semi transparent overlays.
pub struct ColorBox {
    pub color: [f32; 4],
}

impl<T> Drawable<T> for ColorBox
where
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<T>, sp: SpacialProperties) -> DrawResult {
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

impl<T> Drawable<T> for TextureBox
where
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<T>, sp: SpacialProperties) -> DrawResult {
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
                in_image: &self.texture,
            },
        }.draw(params, sp)
    }
}
