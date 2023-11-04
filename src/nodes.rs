#[cfg(target_os = "linux")]
use crate::nodes_gpu::display::Display;
#[cfg(target_os = "linux")]
use crate::nodes_gpu::plot::Plot;
#[cfg(target_os = "linux")]
use crate::nodes_io::reader_webcam::WebcamInput;
use crate::{
    nodes_cpu::{
        //average::Average,
        benchmark_sink::BenchmarkSink,
        dual_frame_raw_decoder::{DualFrameRawDecoder, ReverseDualFrameRawDecoder},
        fp_to_uint::Fp32ToUInt16,
        //sz3::SZ3Compress,
        zstd::ZstdBlobReader,
    },
    nodes_gpu::{
        calibrate::Calibrate,
        color_voodoo::ColorVoodoo,
        debayer::Debayer,
        histogram::Histogram,
        lut_3d::Lut3d,
    },
    nodes_io::{
        reader_cinema_dng::CinemaDngReader,
        reader_raw::{RawBlobReader, RawDirectoryReader},
        reader_tcp::TcpReader,
        writer_cinema_dng::CinemaDngWriter,
        writer_raw::{RawBlobWriter, RawDirectoryWriter},
    },
    nodes_util::{cache::Cache, split::Split},
    pipeline_processing::{
        node::{Node, NodeID, ProcessingNodeIntoNode, SinkNodeIntoNode},
        parametrizable::prelude::*,
        processing_context::ProcessingContext,
    },
};
use crate::{
    nodes_gpu::base_gpu_node::GpuNodeImpl,
    nodes_io::{frameserver_cinema_dng::CinemaDngFrameserver, writer_ffmpeg::FfmpegWriter},
    nodes_util::null_source::NullFrameSource,
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

        pub fn create_node_from_name(name: &str, node_id: NodeID, parameters: Parameters, inputs: HashMap<String, Node>, is_input_to: &[NodeID], context: &ProcessingContext) -> Result<Node> {
            $(
                $(#[$m])?
                if name == <$x>::get_name() {
                    let parameters = parameters.add_inputs(node_id, inputs)?;
                    let parameters = parameters.add_defaults(<$x>::describe_parameters());
                    return Ok(<$x>::from_parameters(parameters, is_input_to, &context)?.into_processing_element())
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
    CinemaDngReader,
    GpuNodeImpl<Debayer>,
    GpuNodeImpl<ColorVoodoo>,
    GpuNodeImpl<Lut3d>,
    #[cfg(target_os = "linux")]
    Display,
    DualFrameRawDecoder,
    ReverseDualFrameRawDecoder,
    BenchmarkSink,
    RawDirectoryWriter,
    RawBlobWriter,
    //Average,
    TcpReader,
    Cache,
    Split,
    //SZ3Compress,
    ZstdBlobReader,
    Calibrate,
    Histogram,
    #[cfg(target_os = "linux")]
    Plot,
    #[cfg(target_os = "linux")]
    WebcamInput,
    FfmpegWriter,
    CinemaDngFrameserver,
    NullFrameSource,
    Fp32ToUInt16,
];
