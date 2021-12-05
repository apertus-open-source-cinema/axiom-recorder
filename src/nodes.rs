use crate::{
    nodes_io::{reader_raw::RawDirectoryReader, writer_cinema_dng::CinemaDngWriter},
    pipeline_processing::{
        node::{
            ProcessingElement,
            ProcessingNodeIntoProcessingElement,
            ProcessingSinkIntoProcessingElement,
        },
        parametrizable::{Parameterizable, ParameterizableDescriptor, Parameters},
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

        pub fn create_node_from_name(name: &str, parameters: &Parameters) -> Result<ProcessingElement> {
            $(
                if name == <$x>::get_name() {
                    return Ok(<$x>::from_parameters(parameters)?.into_processing_element())
                };
            )+

            Err(anyhow!("no node named {} found", name))
        }
    };
}

generate_dynamic_node_creation_functions![RawDirectoryReader, CinemaDngWriter,];
