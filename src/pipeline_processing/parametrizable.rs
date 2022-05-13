use self::ParameterTypeDescriptor::{Mandatory, Optional};
use anyhow::{anyhow, Context, Error, Result};
use std::{any::type_name, convert::TryInto};

use crate::pipeline_processing::{
    frame::{CfaDescriptor, FrameInterpretations, Raw, Rgb},
    node::{InputProcessingNode, Node, NodeID},
    processing_context::ProcessingContext,
};
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
};

pub enum ParameterValue {
    FloatRange(f64),
    IntRange(i64),
    StringParameter(String),
    BoolParameter(bool),
    NodeInput(InputProcessingNode),
}
impl ParameterValue {
    fn clone_for_same_puller(&self) -> Self {
        match self {
            Self::FloatRange(f) => Self::FloatRange(*f),
            Self::IntRange(i) => Self::IntRange(*i),
            Self::BoolParameter(b) => Self::BoolParameter(*b),
            Self::StringParameter(s) => Self::StringParameter(s.clone()),
            Self::NodeInput(n) => Self::NodeInput(n.clone_for_same_puller()),
        }
    }
}

impl ToString for ParameterValue {
    fn to_string(&self) -> String {
        match self {
            Self::FloatRange(v) => v.to_string(),
            Self::IntRange(v) => v.to_string(),
            Self::StringParameter(v) => v.to_string(),
            Self::BoolParameter(v) => v.to_string(),
            Self::NodeInput(_) => "<NodeInput>".to_string(),
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
            Self::FloatRange(v) => Ok(v),
            _ => Err(anyhow!("cant convert a non FloatRange ParameterValue to f64")),
        }
    }
}

impl TryInto<i64> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<i64, Self::Error> {
        match self {
            Self::IntRange(v) => Ok(v),
            _ => Err(anyhow!("cant convert a non IntRange ParameterValue to i64")),
        }
    }
}

impl TryInto<u64> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<u64, Self::Error> {
        match self {
            Self::IntRange(v) => Ok(v as u64),
            _ => Err(anyhow!("cant convert a non IntRange ParameterValue to u64")),
        }
    }
}

impl TryInto<String> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<String, Self::Error> {
        match self {
            Self::StringParameter(v) => Ok(v),
            _ => Err(anyhow!("cant convert a non StringParameter ParameterValue to string")),
        }
    }
}

impl TryInto<bool> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<bool, Self::Error> {
        match self {
            Self::BoolParameter(v) => Ok(v),
            _ => Err(anyhow!("cant convert a non BoolParameter ParameterValue to bool")),
        }
    }
}

impl TryInto<InputProcessingNode> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<InputProcessingNode, Self::Error> {
        match self {
            Self::NodeInput(v) => Ok(v),
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

    pub fn get<T>(&mut self, key: &str) -> Result<T>
    where
        ParameterValue: TryInto<T, Error = anyhow::Error>,
    {
        let parameter_value = self
            .values
            .remove(key)
            .ok_or_else(|| anyhow!("key {} not present in parameter storage", key))?;
        parameter_value.try_into()
    }

    pub(crate) fn add_inputs(
        mut self,
        puller_id: NodeID,
        inputs: HashMap<String, Node>,
    ) -> Result<Self> {
        for (name, node) in inputs {
            self.values.insert(
                name.clone(),
                ParameterValue::NodeInput(InputProcessingNode::new(
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
            if let Optional(_, value) = value {
                self.values.entry(name).or_insert(value);
            }
        }

        self
    }

    pub fn get_interpretation(&mut self) -> Result<FrameInterpretations> {
        let width = self.get("width")?;
        let height = self.get("height")?;
        let bit_depth = self.get("bit-depth")?;
        let cfa = CfaDescriptor::from_first_red(
            self.get("red-in-first-col")?,
            self.get("red-in-first-row")?,
        );
        let fps = self.get("fps")?;

        if self.get("rgb")? {
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
    StringParameter,
    BoolParameter,
    NodeInput,
}

impl ParameterType {
    pub fn value_is_of_type(&self, value: ParameterValue) -> Result<ParameterValue> {
        match (self, &value) {
            (Self::StringParameter, ParameterValue::StringParameter(_)) => Ok(value),
            (Self::BoolParameter, ParameterValue::BoolParameter(_)) => Ok(value),
            (Self::FloatRange(min, max), ParameterValue::FloatRange(v)) => {
                if (v >= min) && (v <= max) {
                    Ok(value)
                } else {
                    Err(anyhow!("value {} is not {} <= value <= {}", v, min, max))
                }
            }
            (Self::IntRange(min, max), ParameterValue::IntRange(v)) => {
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
            Self::StringParameter => Ok(ParameterValue::StringParameter(string.to_string())),
            Self::BoolParameter => Ok(ParameterValue::BoolParameter(string.parse()?)),
            Self::IntRange(..) => self.value_is_of_type(ParameterValue::IntRange(string.parse()?)),
            Self::FloatRange(..) => {
                self.value_is_of_type(ParameterValue::FloatRange(string.parse()?))
            }
            Self::NodeInput => Err(anyhow!("cant parse node input from string")),
        }
    }
}

#[derive(Debug)]
pub enum ParameterTypeDescriptor {
    Optional(ParameterType, ParameterValue),
    Mandatory(ParameterType),
}

impl Clone for ParameterTypeDescriptor {
    fn clone(&self) -> Self {
        match self {
            Self::Optional(ty, v) => Self::Optional(ty.clone(), v.clone_for_same_puller()),
            Self::Mandatory(ty) => Self::Mandatory(ty.clone()),
        }
    }
}

impl ParameterTypeDescriptor {
    pub fn parse(&self, string: Option<&str>) -> Result<ParameterValue> {
        match self {
            Self::Optional(parameter_type, default_value) => string
                .map(|s| parameter_type.parse(s))
                .unwrap_or_else(|| Ok(default_value.clone_for_same_puller())),
            Self::Mandatory(parameter_type) => {
                string.map(|s| parameter_type.parse(s)).unwrap_or_else(|| {
                    Err(anyhow!("parameter was not supplied but is mandatory (no default value)"))
                })
            }
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
        self.with(
            "bit-depth",
            Optional(ParameterType::IntRange(8, 16), ParameterValue::IntRange(12)),
        )
        .with("width", Mandatory(ParameterType::IntRange(0, i64::max_value())))
        .with("height", Mandatory(ParameterType::IntRange(0, i64::max_value())))
        .with(
            "red-in-first-col",
            Optional(ParameterType::BoolParameter, ParameterValue::BoolParameter(true)),
        )
        .with(
            "red-in-first-row",
            Optional(ParameterType::BoolParameter, ParameterValue::BoolParameter(true)),
        )
        .with("rgb", Optional(ParameterType::BoolParameter, ParameterValue::BoolParameter(false)))
        .with(
            "fps",
            Optional(ParameterType::FloatRange(0.0, f64::MAX), ParameterValue::FloatRange(24.0)),
        )
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
