use anyhow::{anyhow, Context, Result};
use clap::{App, AppSettings, Arg};
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use recorder::pipeline_processing::{
    create_node_from_name,
    execute::execute_pipeline,
    list_available_nodes,
    parametrizable::{
        ParameterTypeDescriptor::{Mandatory, Optional},
        ParameterValue,
        ParameterizableDescriptor,
        Parameters,
        VULKAN_CONTEXT,
    },
    payload::Payload,
    processing_node::ProcessingNode,
};
use std::{
    collections::HashMap,
    env,
    iter::{once, FromIterator},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
        MutexGuard,
        RwLock,
    },
    time::SystemTime,
};
use vulkano::{
    device::{physical::PhysicalDevice, Device, DeviceExtensions},
    instance::Instance,
    Version,
};

fn main() {
    let res = work();
    match res {
        Ok(_) => eprintln!("\nconversion successfully finished :)"),
        Err(error) => {
            eprintln!("An error occured: \n{}", error.chain().map(|e| format!("{}", e)).join("\n"))
        }
    }
}

// used to have the convenience of ? for error handling
fn work() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let arg_blocks: Vec<Vec<&String>> = args.split(|s| s == "!").map(Vec::from_iter).collect();

    let _main_app_arguments = App::new("Raw Image / Video Converter")
        .usage("converter [--app-args] ! <VideoSource> --source arg ! <VideoSink> --sink arg")
        .about("convert raw footage from AXIOM cameras into other formats.")
        .after_help(format!("NODES:\n{}", nodes_usages_string()).as_str())
        .get_matches_from(&arg_blocks[0]);

    let vk_context = {
        let extensions = vulkano_win::required_extensions();
        let instance = Instance::new(None, Version::V1_2, &extensions, None)?;
        let (device, queues) = PhysicalDevice::enumerate(&instance)
            .find_map(|physical| {
                let queue_family = physical.queue_families().map(|qf| (qf, 0.5)); // All queues have the same priority
                let device_ext = DeviceExtensions {
                    khr_swapchain: true,
                    khr_storage_buffer_storage_class: true,
                    khr_8bit_storage: true,
                    khr_shader_non_semantic_info: true,
                    ..(*physical.required_extensions())
                };
                Device::new(physical, physical.supported_features(), &device_ext, queue_family).ok()
            })
            .ok_or_else(|| anyhow!("No physical device found"))?;

        println!("using gpu: {}", device.physical_device().properties().device_name);

        ParameterValue::VulkanContext(device, queues.collect())
    };

    let mut nodes = arg_blocks[1..]
        .iter()
        .map(|arg_block| processing_node_from_commandline(vk_context.clone(), arg_block))
        .collect::<Result<Vec<_>>>()?;
    nodes.push(Arc::new(ProgressNode::new(nodes[0].size_hint())));

    execute_pipeline(nodes)?;

    Ok(())
}

struct ProgressNode {
    progressbar: ProgressBar,
    start_time: RwLock<SystemTime>,
    fps_counter: AtomicU64,
}
impl ProgressNode {
    fn new(total: Option<u64>) -> ProgressNode {
        let progressbar = match total {
            Some(n) => {
                let bar = ProgressBar::new(n as u64);
                bar.set_style(ProgressStyle::default_bar()
                    .template("{wide_bar} | {pos}/{len} frames | elapsed: {elapsed_precise} | remaining: {eta} | {msg} ")
                    .progress_chars("#>-"));
                bar
            }
            None => ProgressBar::new_spinner(),
        };

        ProgressNode {
            progressbar,
            start_time: RwLock::new(SystemTime::now()),
            fps_counter: AtomicU64::new(0),
        }
    }
}
impl ProcessingNode for ProgressNode {
    fn process(
        &self,
        _input: &mut Payload,
        _frame_lock: MutexGuard<u64>,
    ) -> Result<Option<Payload>> {
        self.progressbar.inc(1);
        self.fps_counter.fetch_add(1, Ordering::Relaxed);

        let time = SystemTime::now();
        let elapsed = time.duration_since(*self.start_time.read().unwrap()).unwrap().as_secs_f64();
        if elapsed > 1.0 {
            self.progressbar.set_message(format!(
                "{:.1} fps",
                self.fps_counter.swap(0, Ordering::Relaxed) as f64 / elapsed
            ));
            *self.start_time.write().unwrap() = time;
        }
        Ok(Some(Payload::empty()))
    }
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
        if key == VULKAN_CONTEXT {
            continue;
        }
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
    vk_context: ParameterValue,
    commandline: &[&String],
) -> Result<Arc<dyn ProcessingNode>> {
    let name = commandline[0];

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
        .filter(|(key, _)| key != &VULKAN_CONTEXT)
        .map(|(key, parameter_type)| {
            Ok((key.to_string(), parameter_type.parse(results.value_of(key))?))
        })
        .collect::<Result<_, anyhow::Error>>()?;

    parameters.insert(VULKAN_CONTEXT.to_string(), vk_context);

    create_node_from_name(name, &Parameters(parameters))
        .with_context(|| format!("Error while creating Node {}", name))
}
