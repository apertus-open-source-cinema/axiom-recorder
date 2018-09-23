use crate::graphical::ui_lib::*;
use std::ops::Deref;

/// The most generic list container. If you want to draw multiple things, use this.
/// Every Drawable is drawn to its position relative to the container position
impl<D, T> Drawable<T> for Vec<(D, SpatialProperties)>
where
    D: Deref<Target = Drawable<T>>,
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, T>, sp: SpatialProperties) -> DrawResult {
        for (drawable, child_sp) in self {
            let absolute_sp = SpatialProperties {
                start: Vec2 {
                    x: (child_sp.start.x * sp.size.x) + sp.start.x,
                    y: (child_sp.start.y * sp.size.y) + sp.start.y,
                },
                size: Vec2 {
                    x: child_sp.size.x * sp.size.x,
                    y: child_sp.size.y * sp.size.y,
                },
            };

            drawable.draw(params, absolute_sp)?
        }
        Ok(())
    }
}

/// A less generalized form of the Vec container, here each element is drawn with full width and height.
default impl<D, T> Drawable<T> for Vec<D>
where
    D: Deref<Target = Drawable<T>>,
    T: Surface,
{
    default fn draw(&self, params: &mut DrawParams<'_, T>, sp: SpatialProperties) -> DrawResult {
        let vec_with_size: Vec<_> = self
            .into_iter()
            .map(|elem| (*elem, SpatialProperties::full()))
            .collect();
        vec_with_size.draw(params, sp)
    }
}
