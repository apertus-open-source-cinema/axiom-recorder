use crate::pipeline_processing::{
    frame::{CfaDescriptor, FrameInterpretations, Raw, Rgb},
    node::{InputProcessingNode, Node, NodeID},
    processing_context::ProcessingContext,
};
use anyhow::{anyhow, Context, Error, Result};
use prelude::*;
use std::{
    any::type_name,
    collections::HashMap,
    convert::TryInto,
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
impl ParameterValue {
    fn clone_for_same_puller(&self) -> Self {
        match self {
            FloatRangeValue(f) => Self::FloatRangeValue(*f),
            IntRangeValue(i) => Self::IntRangeValue(*i),
            BoolValue(b) => Self::BoolValue(*b),
            StringValue(s) => Self::StringValue(s.clone()),
            ListValue(l) => {
                Self::ListValue(l.iter().map(ParameterValue::clone_for_same_puller).collect())
            }
            Self::NodeInputValue(n) => Self::NodeInputValue(n.clone_for_same_puller()),
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
            _ => Err(anyhow!("cant convert a non FloatRange ParameterValue to f64")),
        }
    }
}

impl TryInto<i64> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<i64, Self::Error> {
        match self {
            IntRangeValue(v) => Ok(v),
            _ => Err(anyhow!("cant convert a non IntRange ParameterValue to i64")),
        }
    }
}

impl TryInto<u64> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<u64, Self::Error> {
        match self {
            IntRangeValue(v) => Ok(v as u64),
            _ => Err(anyhow!("cant convert a non IntRange ParameterValue to u64")),
        }
    }
}


impl TryInto<usize> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<usize, Self::Error> {
        match self {
            IntRangeValue(v) => Ok(v as usize),
            _ => Err(anyhow!("cant convert a non IntRange ParameterValue to u64")),
        }
    }
}

impl TryInto<u8> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<u8, Self::Error> {
        match self {
            IntRangeValue(v) => Ok(v as u8),
            _ => Err(anyhow!("cant convert a non IntRange ParameterValue to u8")),
        }
    }
}

impl TryInto<String> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<String, Self::Error> {
        match self {
            StringValue(v) => Ok(v),
            _ => Err(anyhow!("cant convert a non StringParameter ParameterValue to string")),
        }
    }
}

impl TryInto<bool> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<bool, Self::Error> {
        match self {
            BoolValue(v) => Ok(v),
            _ => Err(anyhow!("cant convert a non BoolParameter ParameterValue to bool")),
        }
    }
}

impl TryInto<InputProcessingNode> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<InputProcessingNode, Self::Error> {
        match self {
            NodeInputValue(v) => Ok(v),
            _ => Err(anyhow!("cant convert a non NodeInput ParameterValue to ProcessingNode")),
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
        ParameterValue: TryInto<T, Error = anyhow::Error>,
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
        ParameterValue: TryInto<T, Error = anyhow::Error>,
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

    pub fn get_interpretation(&mut self) -> Result<FrameInterpretations> {
        let width = self.take("width")?;
        let height = self.take("height")?;
        let bit_depth = self.take("bit-depth")?;
        let cfa = CfaDescriptor::from_first_red(
            self.take("red-in-first-col")?,
            self.take("red-in-first-row")?,
        );
        let fps = self.take("fps")?;

        if self.take("rgb")? {
            Ok(FrameInterpretations::Rgb(Rgb { width, height, fps }))
        } else {
            Ok(FrameInterpretations::Raw(Raw { bit_depth, width, height, cfa, fps }))
        }
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
            (StringParameter, ParameterValue::StringValue(_)) => Ok(value),
            (BoolParameter, ParameterValue::BoolValue(_)) => Ok(value),
            (FloatRange(min, max), ParameterValue::FloatRangeValue(v)) => {
                if (v >= min) && (v <= max) {
                    Ok(value)
                } else {
                    Err(anyhow!("value {} is not {} <= value <= {}", v, min, max))
                }
            }
            (IntRange(min, max), ParameterValue::IntRangeValue(v)) => {
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
            StringParameter => Ok(ParameterValue::StringValue(string.to_string())),
            BoolParameter => Ok(ParameterValue::BoolValue(string.parse()?)),
            IntRange(..) => self.value_is_of_type(ParameterValue::IntRangeValue(string.parse()?)),
            FloatRange(..) => {
                self.value_is_of_type(ParameterValue::FloatRangeValue(string.parse()?))
            }
            NodeInputParameter => Err(anyhow!("cant parse node input from string")),
            ListParameter(ty) => {
                let values = if string.trim().is_empty() {
                    vec![]
                } else {
                    string.split(',').map(|part| ty.parse(part)).collect::<Result<_>>()?
                };
                Ok(ParameterValue::ListValue(values))
            }
        }
    }
    pub fn default_value(&self) -> ParameterValue {
        match &self {
            FloatRange(min, _) => FloatRangeValue(*min),
            IntRange(min, _) => IntRangeValue(*min),
            ListParameter(_) => ListValue(vec![]),
            StringParameter => StringValue("".to_string()),
            BoolParameter => BoolValue(false),
            NodeInputParameter => panic!("no default value for node input"),
        }
    }
}

#[derive(Debug)]
pub enum ParameterTypeDescriptor {
    Mandatory(ParameterType),
    WithDefault(ParameterType, ParameterValue),
}

impl Clone for ParameterTypeDescriptor {
    fn clone(&self) -> Self {
        match self {
            Mandatory(ty) => Mandatory(ty.clone()),
            WithDefault(ty, v) => WithDefault(ty.clone(), v.clone_for_same_puller()),
        }
    }
}

impl ParameterTypeDescriptor {
    pub fn parse(&self, string: Option<&str>) -> Result<ParameterValue> {
        match self {
            Mandatory(parameter_type) => {
                string.map(|s| parameter_type.parse(s)).unwrap_or_else(|| {
                    Err(anyhow!("parameter was not supplied but is mandatory (no default value)"))
                })
            }
            WithDefault(parameter_type, default_value) => string
                .map(|s| parameter_type.parse(s))
                .unwrap_or_else(|| Ok(default_value.clone_for_same_puller())),
        }
    }
}

#[derive(Debug, Clone)]
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
        self.with("bit-depth", WithDefault(IntRange(8, 16), IntRangeValue(12)))
            .with("width", Mandatory(NaturalWithZero()))
            .with("height", Mandatory(NaturalWithZero()))
            .with("red-in-first-col", WithDefault(BoolParameter, BoolValue(true)))
            .with("red-in-first-row", WithDefault(BoolParameter, BoolValue(true)))
            .with("rgb", Optional(BoolParameter))
            .with("fps", WithDefault(PositiveReal(), FloatRangeValue(24.0)))
    }
}

#[derive(Debug)]
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

    pub fn Optional(ty: ParameterType) -> ParameterTypeDescriptor {
        WithDefault(ty.clone(), ty.default_value())
    }
    pub fn NaturalWithZero() -> ParameterType { IntRange(0, i64::MAX) }
    pub fn NaturalGreaterZero() -> ParameterType { IntRange(1, i64::MAX) }
    pub fn U8() -> ParameterType { IntRange(0, u8::MAX as i64) }
    pub fn PositiveReal() -> ParameterType { FloatRange(0.0, f64::MAX) }
}
