use crate::graphical::ui_lib::*;

/// The most generic list container. If you want to draw multiple things, use this.
/// Every Drawable is drawn to its position relative to the container position
impl<'a, T> Drawable<T> for Vec<(&'a Drawable<T>, SpacialProperties)>
where
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<T>, sp: SpacialProperties) -> DrawResult {
        for (drawable, child_sp) in self {
            let absolute_sp = SpacialProperties {
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
impl<'a, T> Drawable<T> for Vec<&'a Drawable<T>>
where
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<T>, sp: SpacialProperties) -> DrawResult {
        let vec_with_size: Vec<_> = self
            .into_iter()
            .map(|elem| (*elem, SpacialProperties::full()))
            .collect();
        vec_with_size.draw(params, sp)
    }
}
