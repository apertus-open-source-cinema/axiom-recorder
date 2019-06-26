#![feature(type_ascription)]

use std::error;

pub mod graphical;
pub mod video_io;

type ResN = Result<(), Box<dyn error::Error>>;
type Res<T> = Result<T, Box<dyn error::Error>>;
