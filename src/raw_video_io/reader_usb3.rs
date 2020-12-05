use crate::{
    frame::raw_frame::RawFrame,
    pipeline_processing::{
        parametrizable::{
            ParameterType::{IntRange, StringParameter},
            ParameterTypeDescriptor::Mandatory,
            Parameterizable,
            Parameters,
            ParametersDescriptor,
        },
        processing_node::{Payload, ProcessingNode},
    },
};
use anyhow::Result;
use std::{io::Read, net::TcpStream, sync::Mutex};

pub struct Usb3Reader {
    pub ft60x: Mutex<TcpStream>,
    pub width: u64,
    pub height: u64,
    pub bit_depth: u64,
}
impl Parameterizable for Usb3Reader {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("width", Mandatory(IntRange(0, i64::max_value())))
            .with("height", Mandatory(IntRange(0, i64::max_value())))
            .with("bit-depth", Mandatory(IntRange(8, 16)))
    }

    fn from_parameters(parameters: &Parameters) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            ft60x: Mutex::new(unimplemented!()),
            width: parameters.get::<u64>("width")?,
            height: parameters.get::<u64>("height")?,
            bit_depth: parameters.get::<u64>("bit-depth")?,
        })
    }
}
impl ProcessingNode for Usb3Reader {
    fn process(&self, _input: &mut Payload) -> Result<Option<Payload>> {
        unimplemented!();
    }
}
