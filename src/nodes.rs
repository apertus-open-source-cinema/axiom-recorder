#[cfg(linux)]
use crate::nodes_io::reader_webcam::WebcamInput;
use crate::{
    nodes_cpu::{
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

// #[cfg(feature = "gst")]
// use crate::nodes_io::writer_gstreamer::GstWriter;


macro_rules! generate_dynamic_node_creation_functions {
    ($($x:ty),+ $(,)?) => {
        pub fn list_available_nodes() -> HashMap<String, ParameterizableDescriptor> {
            let mut to_return = HashMap::new();
            $(
                to_return.insert(<$x>::get_name(), <$x>::describe());
            )+
            to_return
        }

        pub fn create_node_from_name(name: &str, parameters: &Parameters, context: &ProcessingContext) -> Result<Node> {
            $(
                if name == <$x>::get_name() {
                    return Ok(<$x>::from_parameters(parameters, &context)?.into_processing_element())
                };
            )+

            Err(anyhow!("no node named {} found", name))
        }
    };
}

#[cfg(linux)]
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
    WebcamInput,
    ColorVoodoo,
    RawDirectoryWriter,
    RawBlobWriter,
    Lut3d,
];

#[cfg(not(linux))]
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
];
