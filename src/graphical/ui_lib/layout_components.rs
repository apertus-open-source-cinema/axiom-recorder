use self::Size::{Percent, Px};
use crate::{graphical::ui_lib::*, ResN};

/// Makes a given child keep the given aspect ratio independent of the aspect
/// ratio of this container. letterboxing of pillarboxing is the result
pub struct AspectRatioContainer<'a, S>
where
    S: Surface,
{
    pub aspect_ratio: f64,
    pub child: &'a dyn Drawable<S>,
}

impl<'a, S> Drawable<S> for AspectRatioContainer<'a, S>
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> ResN {
        let container_ratio =
            (sp.size.x * params.screen_size.x as f64) / (sp.size.y * params.screen_size.y as f64);
        let ratio = container_ratio * (1. / self.aspect_ratio);
        let size = if container_ratio < self.aspect_ratio {
            Vec2 { x: sp.size.x, y: sp.size.y * ratio }
        } else {
            Vec2 { x: sp.size.x / ratio, y: sp.size.y }
        };

        let start = Vec2 { x: (1. - size.x) / 2., y: (1. - size.y) / 2. };

        self.child.draw(params, SpatialProperties { start, size })
    }
}

pub enum Size {
    Px(u32),
    Percent(f64),
}

pub struct SizeContainer<'a, S>
where
    S: Surface,
{
    pub size: Vec2<Size>,
    pub anchor: Vec2<f64>,
    pub child: &'a dyn Drawable<S>,
}

impl<'a, S> Drawable<S> for SizeContainer<'a, S>
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> ResN {
        let size = Vec2 {
            x: match self.size.x {
                Px(px) => sp.size.x * (px as f64 / (params.screen_size.x as f64 * sp.size.x)),
                Percent(percent) => sp.size.x * percent,
            },
            y: match self.size.y {
                Px(px) => sp.size.y * (px as f64 / (params.screen_size.y as f64 * sp.size.y)),
                Percent(percent) => sp.size.y * percent,
            },
        };

        let start = Vec2 {
            x: sp.start.x + (sp.size.x - size.x) * self.anchor.x,
            y: sp.start.y + (sp.size.y - size.y) * self.anchor.y,
        };
        self.child.draw(params, SpatialProperties { start, size })
    }
}


/// draws a drawable at a given location
/// especially usefull in combination with a list container
pub struct LocationContainer<'a, S> {
    pub child: &'a dyn Drawable<S>,
    pub sp: SpatialProperties,
}

impl<'a, S> Drawable<S> for LocationContainer<'a, S>
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> ResN {
        let child_sp = &self.sp;
        let absolute_sp = SpatialProperties {
            start: Vec2 {
                x: (child_sp.start.x * sp.size.x) + sp.start.x,
                y: (child_sp.start.y * sp.size.y) + sp.start.y,
            },
            size: Vec2 { x: child_sp.size.x * sp.size.x, y: child_sp.size.y * sp.size.y },
        };

        self.child.draw(params, absolute_sp)?;

        Ok(())
    }
}
