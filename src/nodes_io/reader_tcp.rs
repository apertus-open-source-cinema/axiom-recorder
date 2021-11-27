use crate::pipeline_processing::{
    frame::{Frame, FrameInterpretation, Raw},
    parametrizable::{
        Parameterizable,
        Parameters,
        ParametersDescriptor,
        ParameterType::StringParameter,
        ParameterTypeDescriptor::Mandatory,
    },
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::Result;
use std::{io::Read, net::TcpStream, sync::Mutex};
use crate::pipeline_processing_legacy::execute::ProcessingStageLockWaiter;
use crate::pipeline_processing_legacy::processing_node::ProcessingNode;

pub struct TcpReader {
    pub tcp_connection: Mutex<TcpStream>,
    interp: Raw,
    context: ProcessingContext,
}
impl Parameterizable for TcpReader {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("address", Mandatory(StringParameter))
            .with_raw_interpretation()
    }

    fn from_parameters(parameters: &Parameters, context: ProcessingContext) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            tcp_connection: Mutex::new(TcpStream::connect(parameters.get::<String>("address")?)?),
            interp: parameters.get_raw_interpretation()?,
            context,
        })
    }
}
impl ProcessingNode for TcpReader {
    fn process(
        &self,
        _input: &mut Payload,
        _frame_lock: ProcessingStageLockWaiter,
    ) -> Result<Option<Payload>> {
        let mut buf = unsafe { self.context.get_uninit_cpu_buffer(self.interp.required_bytes()) };
        buf.as_mut_slice(|slice| self.tcp_connection.lock().unwrap().read_exact(slice))?;
        Ok(Some(Payload::from(Frame { storage: buf, interp: self.interp })))
    }
}
