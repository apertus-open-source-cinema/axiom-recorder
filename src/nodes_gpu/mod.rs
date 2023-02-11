pub mod calibrate;
pub mod color_voodoo;
pub mod debayer;
pub mod lut_3d;

// display and plot are currently only supported on linux because it needs
// EventLoopExtUnix
#[cfg(target_os = "linux")]
pub mod display;

pub mod base_gpu_node;
pub mod histogram;
#[cfg(target_os = "linux")]
pub mod plot;
mod shader_util;
