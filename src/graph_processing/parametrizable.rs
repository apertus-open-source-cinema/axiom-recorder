use self::ParameterTypeDescriptor::{Mandatory, Optional};
use anyhow::{anyhow, Error, Result};
use clap::{App, Arg};
use std::{any::type_name, convert::TryInto};

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum ParameterValue {
    FloatRange(f64),
    IntRange(i64),
    StringParameter(String),
}
impl ToString for ParameterValue {
    fn to_string(&self) -> String {
        match self {
            Self::FloatRange(v) => v.to_string(),
            Self::IntRange(v) => v.to_string(),
            Self::StringParameter(v) => v.to_string(),
        }
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
            _ => Err(anyhow!("cant convert a non IntRange ParameterValue to string")),
        }
    }
}
impl TryInto<u64> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<u64, Self::Error> {
        match self {
            Self::IntRange(v) => Ok(v as u64),
            _ => Err(anyhow!("cant convert a non IntRange ParameterValue to string")),
        }
    }
}
impl TryInto<String> for ParameterValue {
    type Error = Error;

    fn try_into(self) -> Result<String, Self::Error> {
        match self {
            Self::StringParameter(v) => Ok(v),
            _ => Err(anyhow!("cant convert a non StringValue ParameterValue to string")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Parameters(pub HashMap<String, ParameterValue>);
impl Parameters {
    pub fn get<T>(&self, key: &str) -> Result<T>
    where
        ParameterValue: TryInto<T, Error = Error>,
    {
        let parameter_value = self
            .0
            .get(key)
            .ok_or_else(|| anyhow!("key {} not present in parameter storage", key))?;
        Ok(parameter_value.clone().try_into()?)
    }
}

#[derive(Debug, Clone)]
pub enum ParameterType {
    FloatRange(f64, f64),
    IntRange(i64, i64),
    StringParameter,
}
impl ParameterType {
    pub fn value_is_of_type(&self, value: ParameterValue) -> Result<ParameterValue> {
        match (self, &value) {
            (Self::StringParameter, ParameterValue::StringParameter(_)) => Ok(value),
            (Self::FloatRange(min, max), ParameterValue::FloatRange(v)) => {
                if (v >= min) && (v <= max) {
                    Ok(value)
                } else {
                    Err(anyhow!("value {} is not {} <= value <= {}", v, min, v))
                }
            }
            (Self::IntRange(min, max), ParameterValue::IntRange(v)) => {
                if (v >= min) && (v <= max) {
                    Ok(value)
                } else {
                    Err(anyhow!("value {} is not {} <= value <= {}", v, min, v))
                }
            }
            _ => Err(anyhow!("value {:?} has to be of type {:?}", value, self)),
        }
    }

    pub fn parse(&self, string: &str) -> Result<ParameterValue> {
        match self {
            Self::StringParameter => Ok(ParameterValue::StringParameter(string.to_string())),
            Self::IntRange(..) => self.value_is_of_type(ParameterValue::IntRange(string.parse()?)),
            Self::FloatRange(..) => {
                self.value_is_of_type(ParameterValue::FloatRange(string.parse()?))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ParameterTypeDescriptor {
    Optional(ParameterType, ParameterValue),
    Mandatory(ParameterType),
}
impl ParameterTypeDescriptor {
    pub fn parse(&self, string: Option<&str>) -> Result<ParameterValue> {
        match self {
            Self::Optional(parameter_type, default_value) => {
                string.map(|s| parameter_type.parse(s)).unwrap_or_else(|| Ok(default_value.clone()))
            }
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
impl ParametersDescriptor {
    pub fn new() -> ParametersDescriptor { ParametersDescriptor(HashMap::new()) }

    pub fn with(mut self, name: &str, descriptor: ParameterTypeDescriptor) -> ParametersDescriptor {
        self.0.insert(name.to_string(), descriptor);
        ParametersDescriptor(self.0)
    }
}

pub trait Parameterizable {
    const NAME: Option<&'static str> = None;
    const DESCRIPTION: Option<&'static str> = None;

    fn describe_parameters() -> ParametersDescriptor;
    fn from_parameters(parameters: &Parameters) -> Result<Self>
    where
        Self: Sized;

    fn get_name() -> String {
        Self::NAME.map(|v| v.to_string()).unwrap_or_else(|| type_name::<Self>().to_string())
    }

    fn new(parameters: &Parameters) -> Result<Self>
    where
        Self: Sized,
    {
        let mut input_parameters = parameters.0.clone();
        let parameters_description: ParametersDescriptor = Self::describe_parameters();

        let parameters: Result<HashMap<_, _>> = parameters_description
            .0
            .into_iter()
            .map(|(key, parameter_type)| {
                Ok((
                    key.to_string(),
                    match parameter_type {
                        Optional(parameter_type, default_value) => {
                            match input_parameters.remove(&key) {
                                None => parameter_type.value_is_of_type(default_value)?,
                                Some(v) => parameter_type.value_is_of_type(v)?,
                            }
                        }
                        Mandatory(parameter_type) => match input_parameters.remove(&key) {
                            None => {
                                return Err(anyhow!(
                                "parameter {} was not supplied but is mandatory (no default value)",
                                key));
                            }
                            Some(v) => parameter_type.value_is_of_type(v)?,
                        },
                    },
                ))
            })
            .collect();

        if !input_parameters.len() == 0 {
            return Err(anyhow!("bogous input parameters were supplied!"));
        }

        Ok(Self::new(&Parameters(parameters?))?)
    }

    fn from_clap(commandline: Vec<String>) -> Result<Self>
    where
        Self: Sized,
    {
        let mut app = App::new(Self::get_name());
        let description = Self::DESCRIPTION;
        if let Some(ref description) = description {
            app = app.about(*description);
        }

        let parameters_description = Self::describe_parameters();
        for (key, parameter_type) in parameters_description.0.iter() {
            let parameter_type_for_closure = parameter_type.clone();
            app = app.arg(match parameter_type {
                Mandatory(_) => Arg::with_name(key)
                    .long(key)
                    .takes_value(true)
                    .validator(move |v| {
                        parameter_type_for_closure
                            .parse(Some(&v))
                            .map(|_| ())
                            .map_err(|e| format!("{}", e))
                    })
                    .required(true),
                Optional(_, default) => Arg::with_name(key)
                    .long(key)
                    .takes_value(true)
                    .validator(move |v| {
                        parameter_type_for_closure
                            .parse(Some(&v))
                            .map(|_| ())
                            .map_err(|e| format!("{}", e))
                    })
                    .default_value(Box::leak(Box::new(default.to_string())))
                    .required(false),
            })
        }
        let results = app.get_matches_from(commandline);
        let parameters: Result<HashMap<_, _>> = parameters_description
            .0
            .iter()
            .map(|(key, parameter_type)| {
                Ok((key.to_string(), parameter_type.parse(results.value_of(key))?))
            })
            .collect();

        Self::new(&Parameters(parameters?))
    }
}
