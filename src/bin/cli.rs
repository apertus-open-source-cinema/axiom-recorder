use anyhow::{anyhow, Context, Result};
use clap::{Arg, Parser};

use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use recorder::{
    nodes::list_available_nodes,
    pipeline_processing::{
        parametrizable::{
            ParameterType,
            ParameterTypeDescriptor,
            ParameterTypeDescriptor::{Mandatory, Optional},
            ParameterValue,
            ParameterizableDescriptor,
            Parameters,
        },
        processing_context::ProcessingContext,
        processing_graph::{ProcessingGraphBuilder, ProcessingNodeConfig},
    },
};
use serde::{Deserialize, Deserializer};
use std::{
    collections::{BTreeMap, HashMap},
    iter::once,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone)]
enum NodeParam {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    NodeInput(String),
    List(Vec<NodeParam>),
}

impl<'de> Deserialize<'de> for NodeParam {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Debug)]
        #[serde(untagged)]
        enum NodeParamSimple {
            Int(i64),
            Float(f64),
            Bool(bool),
            String(String),
            List(Vec<NodeParam>),
        }

        let param = NodeParamSimple::deserialize(deserializer)?;
        Ok(match param {
            NodeParamSimple::Float(f) => NodeParam::Float(f),
            NodeParamSimple::Int(i) => NodeParam::Int(i),
            NodeParamSimple::String(s) => {
                if let Some(s) = s.strip_prefix('<') {
                    NodeParam::NodeInput(s.to_owned())
                } else {
                    NodeParam::String(s)
                }
            }
            NodeParamSimple::Bool(b) => NodeParam::Bool(b),
            NodeParamSimple::List(l) => NodeParam::List(l),
        })
    }
}

impl TryFrom<NodeParam> for ParameterValue {
    type Error = ();

    fn try_from(value: NodeParam) -> Result<Self, Self::Error> {
        match value {
            NodeParam::Float(f) => Ok(ParameterValue::FloatRange(f)),
            NodeParam::Int(i) => Ok(ParameterValue::IntRange(i)),
            NodeParam::String(s) => Ok(ParameterValue::StringParameter(s)),
            NodeParam::Bool(b) => Ok(ParameterValue::BoolParameter(b)),
            NodeParam::NodeInput(_) => Err(()),
            NodeParam::List(l) => Ok(ParameterValue::ListParameter(
                l.into_iter().map(ParameterValue::try_from).collect::<Result<_, Self::Error>>()?,
            )),
        }
    }
}

#[derive(Deserialize, Debug)]
struct NodeConfig {
    #[serde(rename = "type")]
    ty: String,
    #[serde(flatten)]
    parameters: HashMap<String, NodeParam>,
}

impl From<NodeConfig> for ProcessingNodeConfig<String> {
    fn from(node_config: NodeConfig) -> Self {
        Self {
            name: node_config.ty,
            parameters: Parameters::new(
                node_config
                    .parameters
                    .clone()
                    .into_iter()
                    .filter_map(|(name, param)| match param {
                        NodeParam::NodeInput(_) => None,
                        param => param.try_into().ok().map(|v| (name, v)),
                    })
                    .collect(),
            ),
            inputs: node_config
                .parameters
                .into_iter()
                .filter_map(|(name, param)| match param {
                    NodeParam::NodeInput(i) => Some((name, i)),
                    _ => None,
                })
                .collect(),
        }
    }
}

#[derive(Deserialize, Debug)]
struct PipelineConfig {
    #[serde(flatten)]
    nodes: HashMap<String, NodeConfig>,
}

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    #[cfg(tokio_unstable)]
    console_subscriber::init();
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let res = work();
    match res {
        Ok(_) => eprintln!("\ncli successfully finished :)"),
        Err(error) => {
            eprintln!("\n\n{:?}", error)
        }
    }
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// specify the processing pipeline directly on the cli
    #[clap(after_help = leak(&format!("NODES:\n{}", nodes_usages_string())).as_str())]
    FromCli {
        #[clap(
            help = "example: <Node1> --source-arg ! <Node2> --sink-arg",
            allow_hyphen_values(true)
        )]
        pipeline: Vec<String>,
    },
    /// specify the pipeline configuration from a (yaml) file
    FromFile {
        /// path to the configuration file
        file: std::path::PathBuf,
        /// variables to substitute in the config file
        #[clap(
            short = 's',
            long = "set",
            name = "key=value",
            allow_hyphen_values(true),
            takes_value = true
        )]
        vars: Vec<String>,
    },
}

/// Raw Image / Video Converter
#[derive(Parser, Debug)]
#[clap(
    version,
    about,
    long_about = "convert raw footage from AXIOM cameras into other formats.",
    author
)]
struct Args {
    #[clap(subcommand)]
    command: Command,
    /// show a progress bar
    #[clap(long, short)]
    show_progress: bool,
}

// used to have the convenience of ? for error handling
fn work() -> Result<()> {
    let args = Args::parse();
    let processing_context = ProcessingContext::default();

    let processing_graph = match args.command {
        Command::FromCli { pipeline } => {
            let node_commandlines = pipeline.split(|element| element == "!").collect::<Vec<_>>();
            let mut processing_graph = ProcessingGraphBuilder::new();

            for (i, node_cmd) in node_commandlines.iter().enumerate() {
                processing_graph.add(
                    i,
                    processing_node_from_commandline(
                        node_cmd,
                        if i > 0 { Some(i - 1) } else { None },
                    )?,
                )?;
            }

            processing_graph.build(&processing_context)?
        }
        Command::FromFile { file, vars } => {
            let vars = vars.into_iter().map(|v| {
                let mut split = v.splitn(2, '=');
                let name = split.next().unwrap();
                let value = split.next().ok_or_else(|| anyhow::anyhow!("expected variable name value pair `{v}` to contain atleast one equals (=) sign"))?;
                Ok((name.to_owned(), value.to_owned()))
            }).collect::<Result<BTreeMap<_, _>>>()?;
            let mut handlebars = handlebars::Handlebars::new();
            handlebars.set_strict_mode(true);
            let config: PipelineConfig = serde_yaml::from_str(
                &handlebars.render_template(&std::fs::read_to_string(file)?, &vars)?,
            )?;

            let mut processing_graph = ProcessingGraphBuilder::new();

            for (name, node) in config.nodes {
                processing_graph.add(name, node.into())?;
            }
            processing_graph.build(&processing_context)?
        }
    };


    if args.show_progress {
        let progressbar: Arc<Mutex<Option<ProgressBar>>> = Default::default();

        processing_graph.run(processing_context, move |progress| {
            let mut lock = progressbar.lock().unwrap();
            if lock.is_none() {
                let progressbar = if let Some(total_frames) = progress.total_frames {
                    let bar = ProgressBar::new(total_frames);
                    bar.set_style(ProgressStyle::default_bar()
                        .template("{wide_bar} | {pos}/{len} frames | elapsed: {elapsed_precise} | remaining: {eta} | {msg} ")
                        .progress_chars("#>-"));
                    bar
                } else {
                    ProgressBar::new_spinner()
                };
                *lock = Some(progressbar)
            }
            lock.as_ref().unwrap().set_position(progress.latest_frame);
        })?;
    } else {
        processing_graph.run(processing_context, |_| {})?;
    }

    Ok(())
}

fn nodes_usages_string() -> String {
    list_available_nodes()
        .keys()
        .map(|node_name| {
            Box::leak(Box::new(
                clap_app_from_node_name(node_name)
                    .unwrap()
                    .help_template("    * {usage}")
                    .no_binary_name(true)
                    .try_get_matches_from(once::<&str>("--help"))
                    .unwrap_err()
                    .to_string(),
            ))
        })
        .join("")
}
fn processing_node_from_commandline(
    commandline: &[String],
    input: Option<usize>,
) -> Result<ProcessingNodeConfig<usize>> {
    let name = &commandline[0];

    let available_nodes: HashMap<String, ParameterizableDescriptor> = list_available_nodes();
    let node_descriptor: &ParameterizableDescriptor =
        available_nodes.get(name).ok_or_else(|| {
            anyhow!(
                "cant find node with name {}. avalable nodes are: \n{}",
                name,
                nodes_usages_string()
            )
        })?;
    let parameters_description = &node_descriptor.parameters_descriptor;

    let app = clap_app_from_node_name(name)?;

    let results = app
        .try_get_matches_from(commandline)
        .with_context(|| format!("Wrong Parameters for Node {}", name))?;
    let parameters: HashMap<_, _> = parameters_description
        .0
        .iter()
        .filter(|(_, descriptor)| {
            !matches!(
                descriptor,
                ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput)
                    | ParameterTypeDescriptor::Optional(ParameterType::NodeInput, _)
            )
        })
        .map(|(key, parameter_type)| {
            Ok((
                key.to_string(),
                parameter_type
                    .parse(results.value_of(key))
                    .context(format!("parameter is {}", key))?,
            ))
        })
        .collect::<Result<_, anyhow::Error>>()?;

    Ok(ProcessingNodeConfig::single_input_node(
        name.to_string(),
        Parameters::new(parameters),
        input,
    ))
}

fn leak<T: Clone>(s: &T) -> &'static T { Box::leak(Box::new(s.clone())) }

fn clap_app_from_node_name(name: &str) -> Result<clap::Command<'static>> {
    let available_nodes: HashMap<String, ParameterizableDescriptor> = list_available_nodes();
    let node_descriptor: &ParameterizableDescriptor =
        available_nodes.get(name).ok_or_else(|| {
            anyhow!(
                "cant find node with name {}. avalable nodes are: {:?}",
                name,
                available_nodes.keys()
            )
        })?;

    let mut app = clap::Command::new(node_descriptor.name.clone());
    if let Some(description) = node_descriptor.description.clone() {
        app = app.about(leak(&description).as_str());
    }
    let parameters_description = leak(&node_descriptor.parameters_descriptor);
    for (key, parameter_type) in parameters_description.0.iter() {
        let parameter_type = leak(parameter_type);
        if let ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput)
        | ParameterTypeDescriptor::Optional(ParameterType::NodeInput, _) = parameter_type
        {
            continue;
        };
        let parameter_type_for_closure = parameter_type.clone();
        app = app.arg(match parameter_type {
            Mandatory(_) => Arg::new(leak(&key).as_str())
                .long(key)
                .takes_value(true)
                .allow_hyphen_values(true)
                .validator(move |v| {
                    parameter_type_for_closure
                        .parse(Some(v))
                        .map(|_| ())
                        .map_err(|e| format!("{}", e))
                })
                .required(true),
            Optional(_, default) => Arg::new(key.as_str())
                .long(key)
                .takes_value(true)
                .allow_hyphen_values(true)
                .validator(move |v| {
                    parameter_type_for_closure
                        .parse(Some(v))
                        .map(|_| ())
                        .map_err(|e| format!("{}", e))
                })
                .default_value(Box::leak(Box::new(default.to_string())))
                .required(false),
        })
    }
    Ok(app)
}
