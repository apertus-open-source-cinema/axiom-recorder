use self::Size::{Percent, Px};
use crate::graphical::ui_lib::*;

/// Makes a given child keep the given aspect ratio independent of the aspect ratio of this container.
/// letterboxing of pillarboxing is the result
pub struct AspectRatioContainer<'a, T>
where
    T: Surface + 'a,
{
    pub aspect_ratio: f64,
    pub child: &'a dyn Drawable<T>,
}

impl<'a, T> Drawable<T> for AspectRatioContainer<'a, T>
where
    T: Surface + 'a,
{
    fn draw(&self, params: &mut DrawParams<'_, T>, sp: SpacialProperties) -> DrawResult {
        let container_ratio =
            (sp.size.x * params.screen_size.x as f64) / (sp.size.y * params.screen_size.y as f64);
        let ratio = container_ratio * (1. / self.aspect_ratio);
        let size = if container_ratio < self.aspect_ratio {
            Vec2 {
                x: sp.size.x,
                y: sp.size.y * ratio,
            }
        } else {
            Vec2 {
                x: sp.size.x / ratio,
                y: sp.size.y,
            }
        };

        let start = Vec2 {
            x: (1. - size.x) / 2.,
            y: (1. - size.y) / 2.,
        };

        self.child.draw(params, SpacialProperties { start, size })
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
    pub child: &'a dyn Drawable<T>,
}

impl<'a, T> Drawable<T> for PixelSizeContainer<'a, T>
where
    T: Surface + 'a,
{
    fn draw(&self, params: &mut DrawParams<'_, T>, sp: SpacialProperties) -> DrawResult {
        let size = Vec2 {
            x: match self.resolution.x {
                Px(px) => sp.size.x * (px as f64 / params.screen_size.x as f64),
                Percent(percent) => sp.size.x * percent,
            },
            y: match self.resolution.y {
                Px(px) => sp.size.y * (px as f64 / params.screen_size.y as f64),
                Percent(percent) => sp.size.y * percent,
            },
        };
        self.child.draw(
            params,
            SpacialProperties {
                start: sp.start,
                size,
            },
        )
    }
}
