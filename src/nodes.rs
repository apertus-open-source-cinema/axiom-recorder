#[cfg(target_os = "linux")]
use crate::nodes_io::reader_webcam::WebcamInput;
use crate::{
    nodes_cpu::{
        average::Average,
        benchmark_sink::BenchmarkSink,
        bitdepth_convert::BitDepthConverter,
        dual_frame_raw_decoder::DualFrameRawDecoder,
    },
    nodes_gpu::{
        bitdepth_convert::GpuBitDepthConverter,
        color_voodoo::ColorVoodoo,
        debayer::Debayer,
        display::Display,
        lut_3d::Lut3d,
    },
    nodes_io::{
        reader_raw::{RawBlobReader, RawDirectoryReader},
        reader_tcp::TcpReader,
        writer_cinema_dng::CinemaDngWriter,
        writer_raw::{RawBlobWriter, RawDirectoryWriter},
    },
    pipeline_processing::{
        node::{Node, ProcessingNodeIntoNode, SinkNodeIntoNode},
        parametrizable::{Parameterizable, ParameterizableDescriptor, Parameters},
        processing_context::ProcessingContext,
    },
};
use anyhow::{anyhow, Result};
use std::collections::HashMap;

macro_rules! generate_dynamic_node_creation_functions {
    ($($(#[$m:meta])? $x:ty),+ $(,)?) => {
        pub fn list_available_nodes() -> HashMap<String, ParameterizableDescriptor> {
            let mut to_return = HashMap::new();
            $(
                $(#[$m])?
                to_return.insert(<$x>::get_name(), <$x>::describe());
            )+
            to_return
        }

        pub fn create_node_from_name(name: &str, parameters: &Parameters, context: &ProcessingContext) -> Result<Node> {
            $(
                $(#[$m])?
                if name == <$x>::get_name() {
                    return Ok(<$x>::from_parameters(parameters, &context)?.into_processing_element())
                };
            )+

            Err(anyhow!("no node named {} found", name))
        }
    };
}

generate_dynamic_node_creation_functions![
    RawDirectoryReader,
    RawBlobReader,
    CinemaDngWriter,
    GpuBitDepthConverter,
    Debayer,
    Display,
    BitDepthConverter,
    DualFrameRawDecoder,
    BenchmarkSink,
    ColorVoodoo,
    RawDirectoryWriter,
    RawBlobWriter,
    Lut3d,
    Average,
    TcpReader,
    #[cfg(target_os = "linux")]
    WebcamInput,
];
