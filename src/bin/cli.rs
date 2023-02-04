use anyhow::{anyhow, Context, Result};
use clap::{Arg, Parser};

use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use recorder::{
    nodes::list_available_nodes,
    pipeline_processing::{
        parametrizable::prelude::*,
        processing_context::ProcessingContext,
        processing_graph::{ProcessingGraphBuilder, ProcessingNodeConfig, SerdeNodeConfig},
    },
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashMap},
    iter::once,
    sync::{Arc, Mutex},
};

#[derive(Deserialize, Debug)]
struct PipelineConfig {
    #[serde(flatten)]
    nodes: HashMap<String, SerdeNodeConfig>,
}

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
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
                        .unwrap()
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
        .sorted()
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
    let name = commandline.get(0).ok_or(anyhow!("need to specify at least two nodes for a pipeline\nsee --help for instructions on how to use this tool"))?;

    let available_nodes: HashMap<String, ParameterizableDescriptor> = list_available_nodes();
    let node_descriptor: &ParameterizableDescriptor =
        available_nodes.get(name).ok_or_else(|| {
            anyhow!(
                "cant find node with name {}. available nodes are: \n{}",
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
                Mandatory(NodeInputParameter) | WithDefault(NodeInputParameter, _)
            )
        })
        .filter_map(|(key, parameter_type)| {
            results.value_of(key).map(|v| {
                Ok((
                    key.to_string(),
                    parameter_type
                        .get_parameter_type()
                        .parse(v)
                        .context(format!("parameter is {}", key))?,
                ))
            })
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
                "cant find node with name {}. available nodes are: {:?}",
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
        if let Mandatory(NodeInputParameter) | WithDefault(NodeInputParameter, _) = parameter_type {
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
                        .get_parameter_type()
                        .parse(v)
                        .map(|_| ())
                        .map_err(|e| format!("{}", e))
                })
                .required(true),
            WithDefault(BoolParameter, BoolValue(false)) => {
                Arg::new(key.as_str()).long(key).takes_value(false).required(false)
            }
            WithDefault(_, default) => Arg::new(key.as_str())
                .long(key)
                .takes_value(true)
                .allow_hyphen_values(true)
                .validator(move |v| {
                    parameter_type_for_closure
                        .get_parameter_type()
                        .parse(v)
                        .map(|_| ())
                        .map_err(|e| format!("{}", e))
                })
                .default_value(Box::leak(Box::new(default.to_string())))
                .required(false),
            Optional(_) => Arg::new(key.as_str())
                .long(key)
                .takes_value(true)
                .allow_hyphen_values(true)
                .validator(move |v| {
                    parameter_type_for_closure
                        .get_parameter_type()
                        .parse(v)
                        .map(|_| ())
                        .map_err(|e| format!("{}", e))
                })
                .required(false),
        })
    }
    Ok(app)
}
