pub mod bitdepth_convert;
pub mod darkframe_subtract;
pub mod color_voodoo;
pub mod debayer;
pub mod debayer_resolution_loss;
pub mod lut_3d;

// display and plot are currently only supported on linux because it needs
// EventLoopExtUnix
#[cfg(target_os = "linux")]
pub mod display;

pub mod histogram;
#[cfg(target_os = "linux")]
pub mod plot;
