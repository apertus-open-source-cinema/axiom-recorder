use glium::Surface;
use graphical::ui_lib::basic_components::Size::Percent;
use graphical::ui_lib::basic_components::Size::Px;
use graphical::ui_lib::*;

/// Renders a simple colored Box. Useful for semi transparent overlays.
pub struct ColorBox {
    pub color: [f32; 4],
}

impl<T> Drawable<T> for ColorBox
where
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) -> DrawResult {
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
        }.draw(params, pos)
    }
}

/// The main container. If you want to draw multiple things, use this.
/// Every Drawable is drawn to its position relative to the container position
impl<'a, T> Drawable<T> for Vec<(&'a Drawable<T>, Pos)>
where
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) -> DrawResult {
        for (drawable, drawable_pos) in self {
            // TODO: rethink; check for correctness
            let absolute_pos = Pos {
                start: Vec2 {
                    x: (drawable_pos.start.x * pos.size.x) + pos.start.x,
                    y: (drawable_pos.start.y * pos.size.y) + pos.start.y,
                },
                size: Vec2 {
                    x: drawable_pos.size.x * pos.size.x,
                    y: drawable_pos.size.y * pos.size.y,
                },
            };

            drawable.draw(params, absolute_pos)?
        }
        Ok(())
    }
}

/// A less generalized form of the Vec container, here each element is drawn with full width and height.
impl<'a, T> Drawable<T> for Vec<&'a Drawable<T>>
where
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) -> DrawResult {
        let vec_with_size: Vec<_> = self.into_iter().map(|elem| (*elem, Pos::full())).collect();
        vec_with_size.draw(params, pos)
    }
}

/// Makes a given child keep the given aspect ratio independent of the aspect ratio of this container.
/// letterboxing of pillarboxing is the result
pub struct AspectRatioContainer<'a, T>
where
    T: Surface + 'a,
{
    pub aspect_ratio: f64,
    pub child: &'a Drawable<T>,
}

impl<'a, T> Drawable<T> for AspectRatioContainer<'a, T>
where
    T: Surface + 'a,
{
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) -> DrawResult {
        let container_ratio =
            (pos.size.x * params.screen_size.x as f64) / (pos.size.y * params.screen_size.y as f64);
        let ratio = container_ratio * self.aspect_ratio;
        let size = if container_ratio > self.aspect_ratio {
            Vec2 {
                x: pos.size.x,
                y: pos.size.y * ratio,
            }
        } else {
            Vec2 {
                x: pos.size.x / ratio,
                y: pos.size.y,
            }
        };

        let start = Vec2 {
            x: (1. - size.x) / 2.,
            y: (1. - size.y) / 2.,
        };

        self.child.draw(params, Pos { start, size })
    }
}

// a more advanced container
enum Direction {
    Horizontal,
    Vertical,
}

pub struct EqualDistributingContainer<'a, T>
where
    T: Surface + 'a,
{
    direction: Direction,
    size_hint: f64,
    // TODO: rethink api
    elements: Vec<&'a Drawable<T>>,
}

impl<'a, T> Drawable<T> for EqualDistributingContainer<'a, T>
where
    T: Surface + 'a,
{
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) -> DrawResult {
        unimplemented!()
    }
}

pub enum Size {
    Px(u32),
    Percent(f64),
}

pub struct PixelSizeContainer<'a, T>
where
    T: 'a,
{
    pub resolution: Vec2<Size>,
    pub child: &'a Drawable<T>,
}

impl<'a, T> Drawable<T> for PixelSizeContainer<'a, T>
where
    T: Surface + 'a,
{
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) -> DrawResult {
        let size = Vec2 {
            x: match self.resolution.x {
                Px(px) => pos.size.x * (px as f64 / params.screen_size.x as f64),
                Percent(percent) => pos.size.x * percent,
            },
            y: match self.resolution.y {
                Px(px) => pos.size.y * (px as f64 / params.screen_size.y as f64),
                Percent(percent) => pos.size.y * percent,
            },
        };
        self.child.draw(
            params,
            Pos {
                start: pos.start,
                size,
            },
        )
    }
}
