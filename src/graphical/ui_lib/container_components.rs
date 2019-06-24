use self::EqualDistributingContainer::*;
use crate::graphical::ui_lib::{layout_components::LocationContainer, *};

// a container, that distributes the space evenly between its children
pub enum EqualDistributingContainer<S>
where
    S: Surface,
{
    Horizontal(Vec<Box<dyn Drawable<S>>>),
    Vertical(Vec<Box<dyn Drawable<S>>>),
}

impl<S> Drawable<S> for EqualDistributingContainer<S>
where
    S: Surface,
{
    fn draw(&self, param: &mut DrawParams<'_, S>, sp: SpatialProperties) -> Res {
        let children = match self {
            Horizontal(vec) => vec,
            Vertical(vec) => vec,
        };
        let len = children.len();

        for (i, child) in children.into_iter().enumerate() {
            let container = &LocationContainer {
                child: child.as_ref(),
                sp: SpatialProperties {
                    start: Vec2 { x: (1. / len as f64) * i as f64, y: 0. },
                    size: Vec2 { x: 1. / len as f64, y: 1. },
                },
            };
            (container as &dyn Drawable<_>).draw(param, sp.clone())?
        }

        Ok(())
    }
}
