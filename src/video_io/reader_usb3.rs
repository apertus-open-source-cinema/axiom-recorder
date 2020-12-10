use crate::{
    frame::raw_frame::{CfaDescriptor, RawFrame},
    pipeline_processing::{
        parametrizable::{
            ParameterType::{BoolParameter, IntRange},
            ParameterTypeDescriptor::{Mandatory, Optional},
            ParameterValue,
            Parameterizable,
            Parameters,
            ParametersDescriptor,
        },
        payload::Payload,
        processing_node::ProcessingNode,
    },
};
use anyhow::{anyhow, Result};
use ft60x::ft60x::{FT60x, DEFAULT_PID, DEFAULT_VID};
use std::sync::{Mutex, MutexGuard};

pub struct Usb3Reader {
    pub ft60x: Mutex<FT60x>,
    pub width: u64,
    pub height: u64,
    pub bit_depth: u64,
    pub cfa: CfaDescriptor,
}
impl Parameterizable for Usb3Reader {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
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
            ft60x: Mutex::new(
                FT60x::new(DEFAULT_VID, DEFAULT_PID)
                    .map_err(|_| anyhow!("cant open ft60x maybe try running with sudo?"))?,
            ),
            width: parameters.get::<u64>("width")?,
            height: parameters.get::<u64>("height")?,
            bit_depth: parameters.get::<u64>("bit-depth")?,
            cfa,
        })
    }
}
impl ProcessingNode for Usb3Reader {
    fn process(
        &self,
        _input: &mut Payload,
        _frame_lock: MutexGuard<u64>,
    ) -> Result<Option<Payload>> {
        let padding_len = 2048 * 4 + 1; // the padding is here to allow us to align to full frames
        let mut bytes =
            vec![0u8; (self.width * self.height * self.bit_depth / 8 + padding_len) as usize];
        self.ft60x
            .lock()
            .unwrap()
            .read_exact(&mut bytes)
            .map_err(|_| anyhow!("reading from ft60x failed"))?;
        Ok(Some(Payload::from(RawFrame::from_bytes(
            bytes,
            self.width,
            self.height,
            self.bit_depth,
            self.cfa,
        )?)))
    }
}
