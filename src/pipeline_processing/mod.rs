use crate::{
    nodes_cpu::bitdepth_convert::BitDepthConverter,
    nodes_gpu::{bitdepth_convert::GpuBitDepthConverter, debayer::Debayer, display::Display},
    nodes_io::{
        reader_raw::{RawDirectoryReader},
    },
    pipeline_processing::{
        parametrizable::{Parameterizable, ParameterizableDescriptor, Parameters},
        processing_context::ProcessingContext,
    },
};
use anyhow::{anyhow, Result};
use crate::pipeline_processing_legacy::processing_node::ProcessingNode;
use std::{collections::HashMap, sync::Arc};

// #[cfg(feature = "gst")]
// use crate::nodes_io::writer_gstreamer::GstWriter;

pub mod buffers;
pub mod frame;
pub mod gpu_util;
pub mod parametrizable;
pub mod payload;
pub mod processing_context;
pub mod node;
pub mod executor;

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
    /*RawBlobReader,
    RawDirectoryReader,
    RawBlobWriter,
    RawDirectoryWriter,
    CinemaDngWriter,
    FfmpegWriter,
    GstWriter,
    TcpReader,
     */

    BitDepthConverter,
    Debayer,
    GpuBitDepthConverter,
    Display,
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
