pub mod bitdepth_convert;
pub mod calibrate;
pub mod color_voodoo;
pub mod debayer;
pub mod lut_3d;

// display and plot are currently only supported on linux because it needs
// EventLoopExtUnix
#[cfg(target_os = "linux")]
pub mod display;

#[cfg(target_os = "linux")]
pub mod plot;
pub mod histogram;
