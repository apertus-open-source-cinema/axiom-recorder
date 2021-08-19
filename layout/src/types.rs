use std::ops::Add;

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct BoxConstraints {
    pub min_width: f32,
    pub max_width: f32,
    pub min_height: f32,
    pub max_height: f32,
}

impl BoxConstraints {
    pub fn enforce(&self, other: BoxConstraints) -> Self {
        Self {
            min_width: self.min_width.clamp(other.min_width, other.max_width),
            max_width: self.max_width.clamp(other.min_width, other.max_width),
            min_height: self.min_height.clamp(other.min_height, other.max_height),
            max_height: self.max_height.clamp(other.min_height, other.max_height),
        }
    }

    pub fn constrain(&self, size: Size) -> Size {
        Size {
            width: size.width.clamp(self.min_width, self.max_width),
            height: size.height.clamp(self.min_height, self.max_height),
        }
    }

    pub fn height_is_bounded(&self) -> bool { self.max_height.is_finite() }

    pub fn with_unbounded_height(self) -> Self {
        Self { min_height: 0.0, max_height: f32::INFINITY, ..self }
    }

    pub fn with_tight_height(self, height: f32) -> Self {
        Self { min_height: height, max_height: height, ..self }
    }

    pub fn with_loose_height(self, height: f32) -> Self {
        Self { min_height: 0.0, max_height: height, ..self }
    }


    pub fn with_unbounded_width(self) -> Self {
        Self { min_width: 0.0, max_width: f32::INFINITY, ..self }
    }

    pub fn with_tight_width(self, width: f32) -> Self {
        Self { min_width: width, max_width: width, ..self }
    }

    pub fn with_loose_width(self, width: f32) -> Self {
        Self { min_width: 0.0, max_width: width, ..self }
    }

    pub fn tight_for(size: Size) -> Self {
        Self {
            min_width: size.width,
            max_width: size.width,
            min_height: size.height,
            max_height: size.height,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub fn zero() -> Self { Self { width: 0.0, height: 0.0 } }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Offset {
    pub x: f32,
    pub y: f32,
}

impl Offset {
    pub fn zero() -> Self { Self { x: 0.0, y: 0.0 } }
}

impl Add<Offset> for Offset {
    type Output = Self;

    fn add(self, rhs: Offset) -> Self::Output { Offset { x: self.x + rhs.x, y: self.y + rhs.y } }
}
