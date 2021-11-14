use crate::{
    nodes_cpu::bitdepth_convert::BitDepthConverter,
    nodes_gpu::{bitdepth_convert::GpuBitDepthConverter, debayer::Debayer, display::Display},
    nodes_io::{
        reader_raw::{RawBlobReader, RawDirectoryReader},
        reader_tcp::TcpReader,
        writer_cinema_dng::CinemaDngWriter,
        writer_ffmpeg::FfmpegWriter,
        writer_raw::{RawBlobWriter, RawDirectoryWriter},
    },
    pipeline_processing::{
        parametrizable::{Parameterizable, ParameterizableDescriptor, Parameters},
        processing_context::ProcessingContext,
    },
};
use anyhow::{anyhow, Result};
use processing_node::ProcessingNode;
use std::{collections::HashMap, sync::Arc};

#[cfg(feature = "gst")]
use crate::nodes_io::writer_gstreamer::GstWriter;

pub mod buffers;
pub mod execute;
pub mod frame;
pub mod gpu_util;
pub mod parametrizable;
pub mod payload;
pub mod processing_context;
pub mod processing_node;

macro_rules! generate_dynamic_node_creation_functions {
    ($($x:ty),+ $(,)?) => {
        pub fn list_available_nodes() -> HashMap<String, ParameterizableDescriptor> {
            let mut to_return = HashMap::new();
            $(
                to_return.insert(<$x>::get_name(), <$x>::describe());
            )+
            to_return
        }

        pub fn create_node_from_name(name: &str, parameters: &Parameters, context: ProcessingContext) -> Result<Arc<dyn ProcessingNode>> {
            $(
                if name == <$x>::get_name() {
                    return Ok(Arc::new(<$x>::from_parameters(parameters, context)?))
                };
            )+

            Err(anyhow!("no node named {} found", name))
        }
    };
}


// TODO(robin): this is stupid
#[cfg(feature = "gst")]
generate_dynamic_node_creation_functions![
    //Usb3Reader,
    RawBlobReader,
    RawDirectoryReader,
    BitDepthConverter,
    Debayer,
    RawBlobWriter,
    RawDirectoryWriter,
    CinemaDngWriter,
    FfmpegWriter,
    GstWriter,
    Display,
    TcpReader,
    GpuBitDepthConverter,
];


#[cfg(not(feature = "gst"))]
generate_dynamic_node_creation_functions![
    //Usb3Reader,
    RawBlobReader,
    RawDirectoryReader,
    BitDepthConverter,
    Debayer,
    RawBlobWriter,
    RawDirectoryWriter,
    CinemaDngWriter,
    FfmpegWriter,
    Display,
    TcpReader,
    GpuBitDepthConverter,
];
