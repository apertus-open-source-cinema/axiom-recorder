use crate::{
    graph_processing::{
        parametrizable::{Parameters},
    },
};
use anyhow::{Result};
use processing_node::ProcessingNode;


pub mod parametrizable;
pub mod processing_node;


fn create_node_from_name(
    _node_name: String,
    _options: Parameters,
) -> Result<Box<dyn ProcessingNode>> {
    unimplemented!()
}
