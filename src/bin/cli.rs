use anyhow::{anyhow, Context, Result};
use clap::{App, AppSettings, Arg};

use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use recorder::{
    nodes::{create_node_from_name, list_available_nodes},
    pipeline_processing::{
        node::Node,
        parametrizable::{
            ParameterType,
            ParameterTypeDescriptor,
            ParameterTypeDescriptor::{Mandatory, Optional},
            ParameterValue,
            ParameterizableDescriptor,
            Parameters,
        },
        processing_context::ProcessingContext,
    },
};
use std::{
    collections::HashMap,
    iter::once,
    mem,
    sync::{Arc, Mutex},
};

fn main() {
    let res = work();
    match res {
        Ok(_) => eprintln!("\ncli successfully finished :)"),
        Err(error) => {
            eprintln!("\n\n{:?}", error)
        }
    }
}

// used to have the convenience of ? for error handling
fn work() -> Result<()> {
    let main_app_arguments = App::new("Raw Image / Video Converter")
        .about("convert raw footage from AXIOM cameras into other formats.")
        .setting(AppSettings::TrailingVarArg)
        .arg(
            Arg::with_name("pipeline")
                .required(true)
                .multiple(true)
                .help("example: <Node1> --source-arg ! <Node2> --sink-arg"),
        )
        .after_help(format!("NODES:\n{}", nodes_usages_string()).as_str())
        .get_matches();

    let pipeline_raw = main_app_arguments.values_of_lossy("pipeline").unwrap();
    let pipeline_split =
        if pipeline_raw.len() == 1 { shellwords::split(&pipeline_raw[0])? } else { pipeline_raw };
    let node_commandlines = pipeline_split.split(|element| element == "!").collect::<Vec<_>>();

    let processing_context = ProcessingContext::default();

    let mut last_element = None;
    for node_cmd in node_commandlines {
        let last_taken = mem::take(&mut last_element);
        let node =
            processing_node_from_commandline(node_cmd, &processing_context, last_taken)?;
        last_element = Some(node);
    }

    let sink = if let Node::Sink(sink) = last_element.unwrap() {
        sink
    } else {
        return Err(anyhow!("the last processing element needs to be a sink!"));
    };

    let progressbar: Arc<Mutex<Option<ProgressBar>>> = Default::default();

    pollster::block_on(sink.run(&processing_context, Arc::new(move |progress| {
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
    })))?;

    Ok(())
}

fn nodes_usages_string() -> String {
    list_available_nodes()
        .keys()
        .map(|node_name| {
            Box::leak(Box::new(
                clap_app_from_node_name(node_name)
                    .unwrap()
                    .template("    * {usage}")
                    .setting(AppSettings::NoBinaryName)
                    .get_matches_from_safe(once::<&str>("--help"))
                    .err()
                    .unwrap()
                    .message,
            ))
        })
        .join("\n")
}
fn processing_node_from_commandline(
    commandline: &[String],
    context: &ProcessingContext,
    last_node: Option<Node>,
) -> Result<Node> {
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
        .get_matches_from_safe(commandline)
        .with_context(|| format!("Wrong Parameters for Node {}", name))?;
    let mut parameters: HashMap<_, _> = parameters_description
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

    if let Some(last_node) = last_node {
        if let Node::Node(last_node) = last_node {
            parameters.insert("input".to_string(), ParameterValue::NodeInput(last_node));
        } else {
            return Err(anyhow!("cant use sink as non last element!"));
        }
    }

    create_node_from_name(name, &Parameters(parameters), context)
        .with_context(|| format!("Error while creating Node {}", name))
}

fn clap_app_from_node_name(name: &str) -> Result<App<'static, 'static>> {
    let available_nodes: HashMap<String, ParameterizableDescriptor> = list_available_nodes();
    let node_descriptor: ParameterizableDescriptor = available_nodes
        .get(name)
        .ok_or_else(|| {
            anyhow!(
                "cant find node with name {}. avalable nodes are: {:?}",
                name,
                available_nodes.keys()
            )
        })?
        .clone();

    let mut app = App::new(node_descriptor.name);
    if let Some(description) = node_descriptor.description {
        app = app.about(Box::leak(Box::new(description)).as_str());
    }
    let parameters_description = node_descriptor.parameters_descriptor;
    for (key, parameter_type) in Box::leak(Box::new(parameters_description.0)).iter() {
        if let ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput)
        | ParameterTypeDescriptor::Optional(ParameterType::NodeInput, _) = parameter_type
        {
            continue;
        };
        let parameter_type_for_closure = parameter_type.clone();
        app = app.arg(match parameter_type {
            Mandatory(_) => Arg::with_name(key)
                .long(key)
                .takes_value(true)
                .allow_hyphen_values(true)
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
                .allow_hyphen_values(true)
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
    Ok(app)
}
