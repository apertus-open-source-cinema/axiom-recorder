use crate::{BoxConstraints, Layoutable, LayoutableChild, LayoutableChildren, Offset, Size};
use std::any::Any;

pub mod column;
pub use column::*;
pub mod row;
pub use row::*;

#[derive(Debug)]
pub struct Flex {
    pub flex: f32,
    pub fit: FlexFit,
}

#[derive(Debug)]
pub enum FlexFit {
    Tight,
    Loose,
}

#[derive(Debug)]
struct FlexibleQuery;

#[derive(Debug)]
pub struct Flexible {
    pub flex: Flex,
}

impl Flexible {
    pub fn get<'a>(child: &LayoutableChild<'a>) -> Option<&'a Flex> {
        child.query(&FlexibleQuery as &dyn Any).and_then(|v| <dyn Any>::downcast_ref(v))
    }
}

impl Layoutable for Flexible {
    fn layout(&self, constraint: BoxConstraints, children: LayoutableChildren) -> Size {
        assert_eq!(children.len(), 1);
        let child = children.into_iter().last().unwrap();
        let size = child.layout(constraint);
        child.set_pos(Offset::zero());
        size
    }

    fn query<'a>(
        &'a self,
        query: &dyn Any,
        children: LayoutableChildren<'a>,
    ) -> Option<&'a dyn Any> {
        if <dyn Any>::downcast_ref::<FlexibleQuery>(query).is_some() {
            Some(&self.flex)
        } else {
            assert_eq!(children.len(), 1);
            (&children).into_iter().last().unwrap().query(query)
        }
    }
}

#[derive(Debug)]
pub enum CrossAxisAlignment {
    Start,
    End,
    Center,
}

impl CrossAxisAlignment {
    pub fn spacing_for(&self, max_width: f32, width: f32) -> f32 {
        match self {
            Self::Start => 0.0,
            Self::End => (max_width - width).min(0.0),
            Self::Center => (max_width - width).min(0.0) / 2.0,
        }
    }
}

#[derive(Debug)]
pub enum MainAxisAlignment {
    Start,
    End,
    Center,
    SpaceAround,
    SpaceBetween,
    SpaceEvenly,
}

impl MainAxisAlignment {
    pub fn spacing_for(&self, total_spacing: f32, num_children: usize) -> (f32, f32) {
        let num_children = num_children as f32;

        match self {
            Self::Start => (0.0, 0.0),
            Self::End => (total_spacing, 0.0),
            Self::Center => (total_spacing / 2., 0.0),
            Self::SpaceAround => {
                let unit = total_spacing / num_children;
                (unit / 2., unit)
            }
            Self::SpaceBetween => {
                if num_children > 1.0 {
                    let unit = total_spacing / (num_children - 1.0);
                    (0.0, unit)
                } else {
                    (0.0, 0.0)
                }
            }
            Self::SpaceEvenly => {
                let unit = total_spacing / (num_children + 1.0);
                (unit, unit)
            }
        }
    }
}

#[derive(Debug)]
pub enum MainAxisSize {
    Min,
    Max,
}
