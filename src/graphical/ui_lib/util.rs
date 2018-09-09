use graphical::ui_lib::*;

/// Renders a simple colored Box. Useful for semi transparent overlays.
pub struct ColorBox {
    pub color: [f32; 4],
}

impl Drawable for ColorBox {
    fn draw(&self, params: &mut DrawParams, pos: Pos) {
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
                in_color: self.color
            },
        }.draw(params, pos);
    }
}

/// The main container. If you want to draw multiple things, use this.
/// Every Drawable is drawn to its position relative to the container position
impl<'a> Drawable for Vec<(&'a Drawable, Pos)> {
    fn draw(&self, params: &mut DrawParams, pos: Pos) {
        for (drawable, drawable_pos) in self {
            // TODO: rethink; check for correctness
            let absolute_pos = Pos {
                start: (drawable_pos.start.0 + pos.start.0, drawable_pos.start.1 + pos.start.1),
                size: (drawable_pos.size.0 * pos.size.0, drawable_pos.size.1 * pos.size.1),
            };

            drawable.draw(params, absolute_pos);
        }
    }
}
