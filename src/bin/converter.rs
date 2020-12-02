use anyhow::{Result, anyhow, Context};
use clap::{App, Arg, AppSettings};
use itertools::__std_iter::FromIterator;
use std::env;
use std::collections::HashMap;
use recorder::graph_processing::processing_node::ProcessingNode;
use recorder::graph_processing::parametrizable::{ParameterizableDescriptor, Parameters};
use recorder::graph_processing::{list_available_nodes, create_node_from_name};
use recorder::graph_processing::parametrizable::ParameterTypeDescriptor::{Mandatory, Optional};
use itertools::Itertools;
use std::iter::once;

fn main() {
    let res = work();
    match res {
        Ok(_) => eprintln!("\nconversion successfully finished :)"),
        Err(error) => eprintln!("An error occured: \n{}", error.chain().map(|e| format!("{}", e)).join("\n")),
    }
}

// used to have the convenience of ? for error handling
fn work() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let arg_blocks: Vec<Vec<&String>> =
        args.split(|s| s == "!").map(|slice| Vec::from_iter(slice)).collect();

    let _main_app_arguments = App::new("Raw Image / Video Converter")
        .usage("converter [--app-args] ! <VideoSource> --source arg ! <VideoSink> --sink arg")
        .about("convert raw footage from AXIOM cameras into other formats.")
        .after_help(format!("NODES:\n{}", nodes_usages_string()).as_str())
        .get_matches_from(&arg_blocks[0]);

    for block in arg_blocks[1..].iter() {
        processing_node_from_commandline(&block)?;
    }

    Ok(())
}

fn clap_app_from_node_name(name: &str) -> Result<App<'static, 'static>> {
    let available_nodes: HashMap<String, ParameterizableDescriptor> = list_available_nodes().clone();
    let node_descriptor: ParameterizableDescriptor = available_nodes
        .get(name)
        .ok_or_else(|| anyhow!("cant find node with name {}. avalable nodes are: {:?}", name, available_nodes.keys()))?.clone();

    let mut app = App::new(node_descriptor.name);
    if let Some(description) = node_descriptor.description {
        app = app.about(Box::leak(Box::new(description.to_string())).as_str());
    }
    let parameters_description = node_descriptor.parameters_descriptor;
    for (key, parameter_type) in Box::leak(Box::new(parameters_description.0)).iter() {
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
    Ok(app)
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
                    .err().unwrap().message
                    .to_string()
            ))
        })
        .join("\n")
}
fn processing_node_from_commandline(commandline: &Vec<&String>) -> Result<Box<dyn ProcessingNode>> {
    let name = commandline[0];

    let available_nodes: HashMap<String, ParameterizableDescriptor> = list_available_nodes();
    let node_descriptor: &ParameterizableDescriptor = available_nodes
        .get(name)
        .ok_or_else(|| anyhow!("cant find node with name {}. avalable nodes are: \n{}", name, nodes_usages_string()))?;
    let parameters_description = &node_descriptor.parameters_descriptor;

    let app = clap_app_from_node_name(name)?;

    let results = app.get_matches_from_safe(commandline).with_context(|| format!("Wrong Parameters for Node {}", name))?;
    let parameters: Result<HashMap<_, _>> = parameters_description
        .0
        .iter()
        .map(|(key, parameter_type)| {
            Ok((key.to_string(), parameter_type.parse(results.value_of(key))?))
        })
        .collect();

    create_node_from_name(name, &Parameters(parameters?))
        .with_context(|| format!("Error while creating Node {}", name))
}
