use crate::{
    frame::raw_frame::{CfaDescriptor, RawFrame},
    pipeline_processing::{
        parametrizable::{
            ParameterType::{BoolParameter, IntRange, StringParameter},
            ParameterTypeDescriptor::{Mandatory, Optional},
            ParameterValue,
            Parameterizable,
            Parameters,
            ParametersDescriptor,
        },
        processing_node::{Payload, ProcessingNode},
    },
};
use anyhow::Result;
use std::{
    io::Read,
    net::TcpStream,
    sync::{Mutex, MutexGuard},
};

pub struct TcpReader {
    pub tcp_connection: Mutex<TcpStream>,
    pub width: u64,
    pub height: u64,
    pub bit_depth: u64,
    pub cfa: CfaDescriptor,
}
impl Parameterizable for TcpReader {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("address", Mandatory(StringParameter))
            .with("width", Mandatory(IntRange(0, i64::max_value())))
            .with("height", Mandatory(IntRange(0, i64::max_value())))
            .with("bit-depth", Mandatory(IntRange(8, 16)))
            .with("first-red-x", Optional(BoolParameter, ParameterValue::BoolParameter(true)))
            .with("first-red-y", Optional(BoolParameter, ParameterValue::BoolParameter(true)))
    }

    fn from_parameters(parameters: &Parameters) -> Result<Self>
    where
        Self: Sized,
    {
        let cfa = CfaDescriptor::from_first_red(
            parameters.get("first-red-x")?,
            parameters.get("first-red-y")?,
        );
        Ok(Self {
            tcp_connection: Mutex::new(TcpStream::connect(parameters.get::<String>("address")?)?),
            width: parameters.get::<u64>("width")?,
            height: parameters.get::<u64>("height")?,
            bit_depth: parameters.get::<u64>("bit-depth")?,
            cfa,
        })
    }
}
impl ProcessingNode for TcpReader {
    fn process(
        &self,
        _input: &mut Payload,
        _frame_lock: MutexGuard<u64>,
    ) -> Result<Option<Payload>> {
        let mut bytes = vec![0u8; (self.width * self.height * self.bit_depth / 8) as usize];
        self.tcp_connection.lock().unwrap().read_exact(&mut bytes)?;
        Ok(Some(Payload::from(RawFrame::from_bytes(
            bytes,
            self.width,
            self.height,
            self.bit_depth,
            self.cfa,
        )?)))
    }
}
