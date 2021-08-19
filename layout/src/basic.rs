use crate::{BoxConstraints, Layoutable, LayoutableChildren, Offset, Size};

#[derive(Debug)]
pub struct SizedBox {
    constraint: BoxConstraints,
}

impl SizedBox {
    pub fn new(size: Size) -> Self { Self { constraint: BoxConstraints::tight_for(size) } }

    pub fn constrained(constraint: BoxConstraints) -> Self { Self { constraint } }
}

impl Layoutable for SizedBox {
    fn layout(&self, constraint: BoxConstraints, children: LayoutableChildren) -> Size {
        assert!(children.len() <= 1);
        if let Some(child) = children.into_iter().last() {
            let size = child.layout(self.constraint.enforce(constraint));
            child.set_pos(Offset::zero());
            size
        } else {
            self.constraint.enforce(constraint).constrain(Size::zero())
        }
    }
}
