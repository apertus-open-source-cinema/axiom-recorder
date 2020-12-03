use crate::{
    pipeline_processing::parametrizable::{Parameterizable, ParameterizableDescriptor, Parameters},
    raw_video_io::{
        reader_raw::{RawBlobReader, RawDirectoryReader},
        writer_cinema_dng::CinemaDngWriter,
        writer_raw_n::{RawBlobWriter, RawDirectoryWriter},
    },
};
use anyhow::{anyhow, Result};
use processing_node::ProcessingNode;
use std::collections::HashMap;


pub mod execute;
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
