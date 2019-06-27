use core::fmt;
use std::error;


pub type ResN = Result<(), Box<dyn error::Error>>;
pub type Res<T> = Result<T, Box<dyn error::Error>>;

#[derive(Debug)]
pub struct Error {
    message: String,
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.message) }
}

impl Error {
    pub fn new(message: String) -> Self { Self { message } }

    pub fn error<T>(message: String) -> Res<T> { Err(Box::new(Self::new(message))) }
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => {Error::new(format!($($arg)+))};
}

#[macro_export]
macro_rules! throw {
    ($($arg:tt)+) => {Error::error(format!($($arg)+))?};
}
