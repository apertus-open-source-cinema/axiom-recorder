use crate::graphical::ui_lib::*;

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
    elements: Vec<&'a dyn Drawable<T>>,
}

impl<'a, T> Drawable<T> for EqualDistributingContainer<'a, T>
where
    T: Surface + 'a,
{
    fn draw(&self, params: &mut DrawParams<'_, T>, sp: SpacialProperties) -> DrawResult {
        unimplemented!()
    }
}
