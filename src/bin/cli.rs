use anyhow::{anyhow, Context, Result};
use clap::{Arg, Command};

use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use recorder::{
    nodes::list_available_nodes,
    pipeline_processing::{
        node::NodeID,
        parametrizable::{
            ParameterType,
            ParameterTypeDescriptor,
            ParameterTypeDescriptor::{Mandatory, Optional},
            ParameterizableDescriptor,
            Parameters,
        },
        processing_context::ProcessingContext,
        processing_graph::{ProcessingGraph, ProcessingNodeConfig},
    },
};
use std::{
    collections::HashMap,
    iter::once,
    sync::{Arc, Mutex},
};

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

// used to have the convenience of ? for error handling
fn work() -> Result<()> {
    let main_app_arguments = Command::new("Raw Image / Video Converter")
        .about("convert raw footage from AXIOM cameras into other formats.")
        .trailing_var_arg(true)
        .arg(
            Arg::new("pipeline")
                .required(true)
                .multiple_values(true)
                .help("example: <Node1> --source-arg ! <Node2> --sink-arg"),
        )
        .after_help(format!("NODES:\n{}", nodes_usages_string()).as_str())
        .get_matches();

    let pipeline_raw: Vec<_> = main_app_arguments.values_of("pipeline").unwrap().collect();
    let pipeline_split = if pipeline_raw.len() == 1 {
        shellwords::split(pipeline_raw[0])?
    } else {
        pipeline_raw.iter().map(|f| f.to_string()).collect()
    };
    let node_commandlines = pipeline_split.split(|element| element == "!").collect::<Vec<_>>();

    let processing_context = ProcessingContext::default();

    let mut processing_graph = ProcessingGraph::new();
    let mut last_element = None;

    for node_cmd in node_commandlines {
        last_element = Some(processing_graph.add(processing_node_from_commandline(
            node_cmd,
            last_element,
            &processing_context,
        )?));
    }

    let progressbar: Arc<Mutex<Option<ProgressBar>>> = Default::default();
    let processing_graph = processing_graph.build(&processing_context)?;

    processing_graph.run(&processing_context, move |progress| {
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
    input: Option<NodeID>,
    _context: &ProcessingContext,
) -> Result<ProcessingNodeConfig> {
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

    /*
    if let Some(last_node) = last_node {
        if let Node::Node(last_node) = last_node {
            parameters.insert("input".to_string(), ParameterValue::NodeInput(last_node));
        } else {
            return Err(anyhow!("cant use sink as non last element!"));
        }
    }

    create_node_from_name(name, &Parameters(parameters), context)
        .with_context(|| format!("Error while creating Node {}", name))
        */
}

fn leaked_thing<T: Clone>(s: &T) -> &'static T { Box::leak(Box::new(s.clone())) }

fn clap_app_from_node_name(name: &str) -> Result<Command<'static>> {
    let available_nodes: HashMap<String, ParameterizableDescriptor> = list_available_nodes();
    let node_descriptor: &ParameterizableDescriptor =
        available_nodes.get(name).ok_or_else(|| {
            anyhow!(
                "cant find node with name {}. avalable nodes are: {:?}",
                name,
                available_nodes.keys()
            )
        })?;

    let mut app = Command::new(node_descriptor.name.clone());
    if let Some(description) = node_descriptor.description.clone() {
        app = app.about(leaked_thing(&description).as_str());
    }
    let parameters_description = leaked_thing(&node_descriptor.parameters_descriptor);
    for (key, parameter_type) in parameters_description.0.iter() {
        let parameter_type = leaked_thing(parameter_type);
        if let ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput)
        | ParameterTypeDescriptor::Optional(ParameterType::NodeInput, _) = parameter_type
        {
            continue;
        };
        let parameter_type_for_closure = parameter_type.clone();
        app = app.arg(match parameter_type {
            Mandatory(_) => Arg::new(leaked_thing(&key).as_str())
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
