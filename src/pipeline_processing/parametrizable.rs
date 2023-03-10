use crate::pipeline_processing::{
    frame::{
        CfaDescriptor,
        ColorInterpretation,
        Compression,
        FrameInterpretation,
        SampleInterpretation,
    },
    node::{InputProcessingNode, Node, NodeID},
    processing_context::ProcessingContext,
};
use anyhow::{anyhow, bail, Context, Error, Result};
use clap::{builder::TypedValueParser, Arg, Command};
use prelude::*;
use std::{
    any::type_name,
    collections::HashMap,
    convert::TryInto,
    ffi::OsStr,
    fmt::{Debug, Formatter},
};

pub enum ParameterValue {
    FloatRangeValue(f64),
    IntRangeValue(i64),
    StringValue(String),
    BoolValue(bool),
    NodeInputValue(InputProcessingNode),
    ListValue(Vec<ParameterValue>),
}
impl Clone for ParameterValue {
    fn clone(&self) -> Self {
        match self {
            FloatRangeValue(f) => FloatRangeValue(*f),
            IntRangeValue(i) => IntRangeValue(*i),
            BoolValue(b) => BoolValue(*b),
            StringValue(s) => StringValue(s.clone()),
            ListValue(l) => ListValue(l.iter().map(ParameterValue::clone).collect()),
            NodeInputValue(n) => NodeInputValue(n.clone_for_same_puller()),
        }
    }
}

impl ToString for ParameterValue {
    fn to_string(&self) -> String {
        match self {
            FloatRangeValue(v) => v.to_string(),
            IntRangeValue(v) => v.to_string(),
            StringValue(v) => v.to_string(),
            BoolValue(v) => v.to_string(),
            ListValue(v) => v.iter().map(ParameterValue::to_string).collect::<Vec<_>>().join(","),
            NodeInputValue(_) => "<NodeInput>".to_string(),
        }
    }
}

impl Debug for ParameterValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("ParameterValue({})", self.to_string()))
    }
}

impl TryInto<f64> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<f64, Self::Error> {
        match self {
            FloatRangeValue(v) => Ok(v),
            _ => Err(anyhow!("cant convert a non FloatRange ParameterValue ({self:?}) to f64")),
        }
    }
}


impl TryInto<f32> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<f32, Self::Error> {
        match self {
            FloatRangeValue(v) => Ok(v as f32),
            _ => Err(anyhow!("cant convert a non FloatRange ParameterValue ({self:?}) to f32")),
        }
    }
}

impl TryInto<i64> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<i64, Self::Error> {
        match self {
            IntRangeValue(v) => Ok(v),
            _ => Err(anyhow!("cant convert a non IntRange ParameterValue ({self:?}) to i64")),
        }
    }
}

impl TryInto<u64> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<u64, Self::Error> {
        match self {
            IntRangeValue(v) => Ok(v as u64),
            _ => Err(anyhow!("cant convert a non IntRange ParameterValue ({self:?}) to u64")),
        }
    }
}


impl TryInto<usize> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<usize, Self::Error> {
        match self {
            IntRangeValue(v) => Ok(v as usize),
            _ => Err(anyhow!("cant convert a non IntRange ParameterValue ({self:?}) to u64")),
        }
    }
}

impl TryInto<u8> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<u8, Self::Error> {
        match self {
            IntRangeValue(v) => Ok(v as u8),
            _ => Err(anyhow!("cant convert a non IntRange ParameterValue ({self:?}) to u8")),
        }
    }
}

impl TryInto<String> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<String, Self::Error> {
        match self {
            StringValue(v) => Ok(v),
            _ => Err(anyhow!(
                "cant convert a non StringParameter ParameterValue ({self:?}) to string"
            )),
        }
    }
}

impl TryInto<bool> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<bool, Self::Error> {
        match self {
            BoolValue(v) => Ok(v),
            _ => Err(anyhow!("cant convert a non BoolParameter ParameterValue ({self:?}) to bool")),
        }
    }
}

impl TryInto<InputProcessingNode> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<InputProcessingNode, Self::Error> {
        match self {
            NodeInputValue(v) => Ok(v),
            _ => Err(anyhow!(
                "cant convert a non NodeInput ParameterValue ({self:?}) to ProcessingNode"
            )),
        }
    }
}


#[derive(Debug)]
pub struct Parameters {
    values: HashMap<String, ParameterValue>,
}

impl Parameters {
    pub fn new(values: HashMap<String, ParameterValue>) -> Self { Self { values } }

    pub fn take<T>(&mut self, key: &str) -> Result<T>
    where
        ParameterValue: TryInto<T, Error = Error>,
    {
        let parameter_value = self
            .values
            .remove(key)
            .ok_or_else(|| anyhow!("key {} not present in parameter storage", key))?;
        parameter_value.try_into()
    }

    // FIXME(robin): workaround to https://github.com/rust-lang/rust/issues/96634
    pub fn take_vec<T>(&mut self, key: &str) -> Result<Vec<T>>
    where
        ParameterValue: TryInto<T, Error = Error>,
    {
        let parameter_value = self
            .values
            .remove(key)
            .ok_or_else(|| anyhow!("key {} not present in parameter storage", key))?;
        match parameter_value {
            ListValue(v) => {
                Ok(v.into_iter().map(ParameterValue::try_into).collect::<Result<_, _>>()?)
            }
            _ => Err(anyhow!("cant convert a non ListParameter ParameterValue to Vec")),
        }
    }

    pub fn take_option<T>(&mut self, key: &str) -> Result<Option<T>>
    where
        ParameterValue: TryInto<T, Error = Error>,
    {
        let parameter_value = self.values.remove(key);
        parameter_value.map(|v| v.try_into()).transpose()
    }

    pub(crate) fn add_inputs(
        mut self,
        puller_id: NodeID,
        inputs: HashMap<String, Node>,
    ) -> Result<Self> {
        for (name, node) in inputs {
            self.values.insert(
                name.clone(),
                ParameterValue::NodeInputValue(InputProcessingNode::new(
                    puller_id,
                    node.assert_input_node().with_context(|| {
                        format!("could not convert input {name} to a input node")
                    })?,
                )),
            );
        }

        Ok(self)
    }

    pub(crate) fn add_defaults(mut self, description: ParametersDescriptor) -> Self {
        for (name, value) in description.0 {
            if let WithDefault(_, value) = value {
                self.values.entry(name).or_insert(value);
            }
        }

        self
    }

    pub fn get_interpretation(&mut self) -> Result<FrameInterpretation> {
        let width = self.take("width")?;
        let height = self.take("height")?;
        let fps = self.take_option("fps")?;

        let sample_interpretation = {
            if let Some(bits) = self.take_option::<u8>("uint-bits")? {
                SampleInterpretation::UInt(bits)
            } else if let Some(true) = self.take_option::<bool>("fp16")? {
                SampleInterpretation::FP16
            } else if let Some(true) = self.take_option::<bool>("fp32")? {
                SampleInterpretation::FP32
            } else {
                bail!("no sample interpretation was specified")
            }
        };

        let color_interpretation = {
            if let Some(pattern) = self.take_option::<String>("bayer")? {
                match pattern.to_uppercase().as_str() {
                    "RGGB" => ColorInterpretation::Bayer(CfaDescriptor {
                        red_in_first_col: true,
                        red_in_first_row: true,
                    }),
                    "GBRG" => ColorInterpretation::Bayer(CfaDescriptor {
                        red_in_first_col: true,
                        red_in_first_row: false,
                    }),
                    "GRBG" => ColorInterpretation::Bayer(CfaDescriptor {
                        red_in_first_col: false,
                        red_in_first_row: true,
                    }),
                    "BGGR" => ColorInterpretation::Bayer(CfaDescriptor {
                        red_in_first_col: false,
                        red_in_first_row: false,
                    }),
                    _ => bail!("couldn't parse CFA Pattern"),
                }
            } else if self.take("rgb")? {
                ColorInterpretation::Rgb
            } else if self.take("rgba")? {
                ColorInterpretation::Rgba
            } else {
                bail!("no color interpretation was specified")
            }
        };

        Ok(FrameInterpretation {
            width,
            height,
            fps,
            color_interpretation,
            sample_interpretation,
            compression: Compression::Uncompressed,
        })
    }
}

#[derive(Debug, Clone)]
pub enum ParameterType {
    FloatRange(f64, f64),
    IntRange(i64, i64),
    ListParameter(Box<ParameterType>),
    StringParameter,
    BoolParameter,
    NodeInputParameter,
}

impl ParameterType {
    pub fn value_is_of_type(&self, value: ParameterValue) -> Result<ParameterValue> {
        match (self, &value) {
            (StringParameter, StringValue(_)) => Ok(value),
            (BoolParameter, BoolValue(_)) => Ok(value),
            (FloatRange(min, max), FloatRangeValue(v)) => {
                if (v >= min) && (v <= max) {
                    Ok(value)
                } else {
                    Err(anyhow!("value {} is not {} <= value <= {}", v, min, max))
                }
            }
            (IntRange(min, max), IntRangeValue(v)) => {
                if (v >= min) && (v <= max) {
                    Ok(value)
                } else {
                    Err(anyhow!("value {} is not {} <= value <= {}", v, min, max))
                }
            }
            _ => Err(anyhow!("value {:?} has to be of type {:?}", value, self)),
        }
    }
    pub fn parse(&self, string: &str) -> Result<ParameterValue> {
        match self {
            StringParameter => Ok(StringValue(string.to_string())),
            BoolParameter => Ok(BoolValue(string.parse()?)),
            IntRange(..) => self.value_is_of_type(IntRangeValue(string.parse()?)),
            FloatRange(..) => self.value_is_of_type(FloatRangeValue(string.parse()?)),
            NodeInputParameter => Err(anyhow!("cant parse node input from string")),
            ListParameter(ty) => {
                let values = if string.trim().is_empty() {
                    vec![]
                } else {
                    string.split(',').map(|part| ty.parse(part)).collect::<Result<_>>()?
                };
                Ok(ListValue(values))
            }
        }
    }
}

impl TypedValueParser for ParameterType {
    type Value = ParameterValue;

    fn parse_ref(
        &self,
        _cmd: &Command,
        _arg: Option<&Arg>,
        value: &OsStr,
    ) -> std::result::Result<Self::Value, clap::Error> {
        Ok(self
            .parse(value.to_str().unwrap())
            .map_err(|e| clap::Error::raw(clap::error::ErrorKind::ValueValidation, e))?)
    }
}

#[derive(Debug)]
pub enum ParameterTypeDescriptor {
    Mandatory(ParameterType),
    Optional(ParameterType),
    WithDefault(ParameterType, ParameterValue),
}

impl Clone for ParameterTypeDescriptor {
    fn clone(&self) -> Self {
        match self {
            Mandatory(ty) => Mandatory(ty.clone()),
            WithDefault(ty, v) => WithDefault(ty.clone(), v.clone()),
            Optional(ty) => Optional(ty.clone()),
        }
    }
}

impl ParameterTypeDescriptor {
    pub fn get_parameter_type(&self) -> &ParameterType {
        match self {
            Mandatory(pt) => pt,
            Optional(pt) => pt,
            WithDefault(pt, _) => pt,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParametersDescriptor(pub HashMap<String, ParameterTypeDescriptor>);

impl Default for ParametersDescriptor {
    fn default() -> Self { Self::new() }
}

impl ParametersDescriptor {
    pub fn new() -> Self { ParametersDescriptor(HashMap::new()) }
    pub fn with(mut self, name: &str, descriptor: ParameterTypeDescriptor) -> ParametersDescriptor {
        self.0.insert(name.to_string(), descriptor);
        ParametersDescriptor(self.0)
    }
    pub fn with_interpretation(self) -> ParametersDescriptor {
        self
            // general metadata
            .with("width", Mandatory(NaturalWithZero()))
            .with("height", Mandatory(NaturalWithZero()))
            .with("fps", WithDefault(PositiveReal(), FloatRangeValue(24.0)))

            // buffer interpretation
            .with("uint-bits", Optional(IntRange(1, 64)))
            .with("fp16", Flag())
            .with("fp32", Flag())

            // color interpretation
            .with("bayer", WithDefault(StringParameter, StringValue("RGGB".to_string())))
            .with("rgb", Flag())
            .with("rgba", Flag())
    }
}

#[derive(Clone, Debug)]
pub struct ParameterizableDescriptor {
    pub name: String,
    pub description: Option<String>,
    pub parameters_descriptor: ParametersDescriptor,
}

pub trait Parameterizable {
    const NAME: Option<&'static str> = None;
    const DESCRIPTION: Option<&'static str> = None;

    fn describe_parameters() -> ParametersDescriptor;
    fn from_parameters(
        parameters: Parameters,
        is_input_to: &[NodeID],
        context: &ProcessingContext,
    ) -> Result<Self>
    where
        Self: Sized;

    fn get_name() -> String {
        Self::NAME
            .map(|v| v.to_string())
            .unwrap_or_else(|| type_name::<Self>().rsplit(':').next().unwrap().to_string())
    }
    fn describe() -> ParameterizableDescriptor {
        ParameterizableDescriptor {
            name: Self::get_name(),
            description: Self::DESCRIPTION.map(|s| s.to_string()),
            parameters_descriptor: Self::describe_parameters(),
        }
    }
}

impl<T: Default> Parameterizable for T {
    fn describe_parameters() -> ParametersDescriptor { ParametersDescriptor::default() }
    fn from_parameters(
        _parameters: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self::default())
    }
}


#[allow(non_snake_case)]
pub mod prelude {
    pub use super::{
        ParameterType::{self, *},
        ParameterTypeDescriptor::{self, *},
        ParameterValue::{self, *},
        Parameterizable,
        ParameterizableDescriptor,
        Parameters,
        ParametersDescriptor,
    };
    pub fn Flag() -> ParameterTypeDescriptor { WithDefault(BoolParameter, BoolValue(false)) }
    pub fn NaturalWithZero() -> ParameterType { IntRange(0, i64::MAX) }
    pub fn NaturalGreaterZero() -> ParameterType { IntRange(1, i64::MAX) }
    pub fn U8() -> ParameterType { IntRange(0, u8::MAX as i64) }
    pub fn PositiveReal() -> ParameterType { FloatRange(0.0, f64::MAX) }
}
