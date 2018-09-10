use graphical::ui_lib::*;
use glium::Surface;

/// Renders a simple colored Box. Useful for semi transparent overlays.
pub struct ColorBox {
    pub color: [f32; 4],
}

impl<T> Drawable<T> for ColorBox where T: Surface {
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) {
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
impl<'a, T> Drawable<T> for Vec<(&'a Drawable<T>, Pos)> where T: Surface {
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) {
        for (drawable, drawable_pos) in self {
            // TODO: rethink; check for correctness
            let absolute_pos = Pos {
                start: (
                    (drawable_pos.start.0 * pos.size.0) + pos.start.0,
                    (drawable_pos.start.1 * pos.size.1) + pos.start.1,
                ),
                size: (
                    drawable_pos.size.0 * pos.size.0,
                    drawable_pos.size.1 * pos.size.1,
                ),
            };

            drawable.draw(params, absolute_pos);
        }
    }
}

/// Makes a given child keep the given aspect ratio independent of the aspect ratio of this container.
/// letterboxing of pillarboxing is the result
pub struct AspectRatioContainer<'a, T> where T: Surface + 'a {
    pub aspect_ratio: f32,
    pub element: &'a Drawable<T>,
}

impl<'a, T> Drawable<T> for AspectRatioContainer<'a, T> where T: Surface + 'a {
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) {
        let ratio_difference = pos.size.0 / pos.size.1 - self.aspect_ratio;
        let size = if ratio_difference > 0. {
            (1., 1. - ratio_difference)
        } else {
            (1. - ratio_difference, 1.)
        };

        self.element.draw(params, Pos { start: (0., 0.), size })
    }
}

// a more advanced container
enum Direction {
    Horizontal,
    Vertical,
}

pub struct EqualDistributingContainer<'a, T> where T: Surface + 'a {
    direction: Direction,
    size_hint: f32,
    // TODO: rethink api
    elements: Vec<&'a Drawable<T>>,
}

impl<'a, T> Drawable<T> for EqualDistributingContainer<'a, T> where T: Surface + 'a {
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) {
        unimplemented!()
    }
}
