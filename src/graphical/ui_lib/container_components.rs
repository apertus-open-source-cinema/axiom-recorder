use self::EqualDistributingContainer::*;
use crate::graphical::ui_lib::*;
use std::ops::Deref;

// a container, that distributes the space evenly between its children
pub enum EqualDistributingContainer<D, T>
where
    D: Deref<Target = Drawable<T>>,
    T: Surface,
{
    Horizontal(Vec<D>),
    Vertical(Vec<D>),
}

impl<D, T> Drawable<T> for EqualDistributingContainer<D, T>
where
    D: Deref<Target = Drawable<T>>,
    T: Surface,
{
    fn draw(&self, param: &mut DrawParams<'_, T>, sp: SpatialProperties) -> DrawResult {
        let children = match self {
            Horizontal(vec) => vec,
            Vertical(vec) => vec,
        };
        let len = children.len();

        let drawable_vec: Vec<_> = children
            .iter()
            .enumerate()
            .map(|(i, child)| {
                (
                    *child,
                    SpatialProperties {
                        start: Vec2 {
                            x: (1. / len as f64) * i as f64,
                            y: 0.,
                        },
                        size: Vec2 {
                            x: 1. / len as f64,
                            y: 1.,
                        },
                    },
                )
            }).collect();
        drawable_vec.draw(param, sp)
    }
}
