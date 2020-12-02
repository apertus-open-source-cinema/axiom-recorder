use crate::{
    graph_processing::{
        parametrizable::{Parameters},
    },
};
use anyhow::{Result, anyhow};
use processing_node::ProcessingNode;
use crate::raw_video_io::reader_raw::{RawBlobReader, RawDirectoryReader};
use crate::raw_video_io::writer_raw_n::{RawBlobWriter, RawDirectoryWriter};
use crate::raw_video_io::writer_cinema_dng::CinemaDngWriter;
use crate::graph_processing::parametrizable::{Parameterizable, ParameterizableDescriptor};
use std::collections::HashMap;


pub mod parametrizable;
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

        pub fn create_node_from_name(name: &str, parameters: &Parameters) -> Result<Box<dyn ProcessingNode>> {
            $(
                if name == <$x>::get_name() {
                    return Ok(Box::new(<$x>::from_parameters(parameters)?))
                };
            )+

            Err(anyhow!("no node named {} found", name))
        }
    };
}

generate_dynamic_node_creation_functions![
    RawBlobReader,
    RawDirectoryReader,

    RawBlobWriter,
    RawDirectoryWriter,
    CinemaDngWriter,
];
