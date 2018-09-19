use glium::Surface;
use graphical::ui_lib::util::Size::Percent;
use graphical::ui_lib::util::Size::Px;
use graphical::ui_lib::*;

/// Renders a simple colored Box. Useful for semi transparent overlays.
pub struct ColorBox {
    pub color: [f32; 4],
}

impl<T> Drawable<T> for ColorBox
where
    T: Surface,
{
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
impl<'a, T> Drawable<T> for Vec<(&'a Drawable<T>, Pos)>
where
    T: Surface,
{
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

/// A less generalized form of the Vec container, here each element is drawn with full width and height.
impl<'a, T> Drawable<T> for Vec<&'a Drawable<T>>
where
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) {
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
    pub aspect_ratio: f32,
    pub child: &'a Drawable<T>,
}

impl<'a, T> Drawable<T> for AspectRatioContainer<'a, T>
where
    T: Surface + 'a,
{
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) {
        let container_ratio = ((pos.size.0 * params.screen_size.0 as f32)
            / (pos.size.1 * params.screen_size.1 as f32));
        let ratio = container_ratio * self.aspect_ratio;
        let size = if container_ratio > self.aspect_ratio {
            (pos.size.0, pos.size.1 * ratio)
        } else {
            (pos.size.0 / ratio, pos.size.1)
        };

        let start = ((1. - size.0) / 2., (1. - size.1) / 2.);

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
    size_hint: f32,
    // TODO: rethink api
    elements: Vec<&'a Drawable<T>>,
}

impl<'a, T> Drawable<T> for EqualDistributingContainer<'a, T>
where
    T: Surface + 'a,
{
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) {
        unimplemented!()
    }
}

pub enum Size {
    Px(u32),
    Percent(f32),
}

pub struct PixelSizeContainer<'a, T>
where
    T: 'a,
{
    pub resolution: (Size, Size),
    pub child: &'a Drawable<T>,
}

impl<'a, T> Drawable<T> for PixelSizeContainer<'a, T>
where
    T: Surface + 'a,
{
    fn draw(&self, params: &mut DrawParams<T>, pos: Pos) {
        let size = (
            match self.resolution.0 {
                Px(px) => pos.size.0 * (px as f32 / params.screen_size.0 as f32),
                Percent(percent) => pos.size.0 * percent,
            },
            match self.resolution.1 {
                Px(px) => pos.size.1 * (px as f32 / params.screen_size.1 as f32),
                Percent(percent) => pos.size.1 * percent,
            },
        );
        self.child.draw(
            params,
            Pos {
                start: pos.start,
                size,
            },
        )
    }
}
