use glium::Surface;
use graphical::ui_lib::*;

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
