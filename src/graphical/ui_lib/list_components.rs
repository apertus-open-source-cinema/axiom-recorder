use crate::graphical::ui_lib::*;
use glium::Surface;

/// A generic list container. If you want to draw multiple things, use this.
impl<S> Drawable<S> for Vec<&Drawable<S>>
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> Res {
        for drawable in self {
            drawable.draw(params, sp.clone())?;
        }

        Ok(())
    }
}
