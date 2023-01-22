use futures::{executor::block_on, FutureExt, StreamExt};
use narui::*;
use recorder::{
    gui::image::{image, ArcPartialEqHelper},
    pipeline_processing::{
        node::{NodeID, Request},
        payload::Payload,
        processing_context::{Priority, ProcessingContext},
        processing_graph::{ProcessingGraph, ProcessingGraphBuilder, SerdeNodeConfig},
    },
};
use std::{
    env,
    sync::{Arc, Mutex},
    thread::spawn,
};

#[widget]
pub fn player(context: &mut WidgetContext) -> Fragment {
    let vulkan_context = context.vulkan_context.clone();
    let frame = context.listenable(None);

    let args: Vec<String> = env::args().collect();
    let files = &args[1];

    let handle = context.effect(
        move |context| {
            let processing_context = ProcessingContext::from_vk_device_queues(
                vulkan_context.device.clone(),
                vulkan_context.queues.clone(),
            );
            let build_graph = || -> anyhow::Result<(ProcessingGraph, NodeID)> {
                let mut graph_builder = ProcessingGraphBuilder::new();
                graph_builder.add(
                    "reader".to_string(),
                    serde_yaml::from_str::<SerdeNodeConfig>(
                            &format!("
                            type: CinemaDngReader
                            file-pattern: {files}
                            "),
                    )?
                    .into(),
                )?;
                graph_builder.add(
                    "converter".to_string(),
                    serde_yaml::from_str::<SerdeNodeConfig>(
                        "
                    type: BitDepthConverter
                    input: <reader
                ",
                    )?
                    .into(),
                )?;
                let debayer = graph_builder.add(
                    "debayer".to_string(),
                    serde_yaml::from_str::<SerdeNodeConfig>(
                        "
                    type: Debayer
                    input: <converter
                ",
                    )?
                    .into(),
                )?;
                let graph = graph_builder.build(&processing_context)?;

                Ok((graph, debayer))
            };
            let (graph, debayer) = build_graph().unwrap();
            let debayer = graph.get_node(debayer).assert_input_node().unwrap();
            let caps = debayer.get_caps();

            let (request_sender, request_receiver) = flume::unbounded::<u64>();
            request_sender.send(0).unwrap();

            let context = context.thread_context();
            spawn(move || {
                block_on(async {
                    let mut todo = futures::stream::FuturesOrdered::new();
                    loop {
                        if !todo.is_empty() {
                            futures::select! {
                                (image, i) = todo.select_next_some() => {
                                    let image: anyhow::Result<Payload> = image;
                                    let image = image.unwrap();
                                    context.shout(frame, Some((i, ArcPartialEqHelper(image.downcast().unwrap()))));
                                },
                                to_pull = request_receiver.recv_async() => {
                                    let _processing_context_inner = processing_context.clone();
                                    let debayer = debayer.clone();
                                    let to_pull = to_pull.unwrap();
                                    todo.push_back(processing_context.spawn(Priority::new(0, to_pull),
                                    async move {
                                        let frame = debayer.pull(Request::new(0, to_pull)).await;
                                        (frame, to_pull)
                                    }.boxed()));
                                }
                            };
                        } else {
                            futures::select! {
                                to_pull = request_receiver.recv_async() => {
                                    let _processing_context_inner = processing_context.clone();
                                    let debayer = debayer.clone();
                                    let to_pull = to_pull.unwrap();
                                    todo.push_back(processing_context.spawn(Priority::new(0, to_pull), async move {
                                        let frame = debayer.pull(Request::new(0, to_pull)).await;
                                        (frame, to_pull)
                                    }.boxed()));
                                }
                            };
                        }
                    }
                });
            });

            (caps, Arc::new(Mutex::new(request_sender)))
        },
        (),
    );

    let (caps, request_sender) = (&*handle.read()).clone();
    let frame_count = caps.frame_count.unwrap_or(1);

    let frame_with_number = context.listen(frame);
    let frame_number = frame_with_number.clone().map_or(0, |(number, _)| number);

    let old_frame_pos = context.listenable(0);
    let frame_pos = context.listenable(0);
    let _frame_pos_v = context.listen(frame_pos);

    context.after_frame(move |context| {
        let frame_pos_v = context.spy(frame_pos);
        let old_frame_pos_v = context.spy(old_frame_pos);
        context.shout(old_frame_pos, frame_pos_v);
        if frame_pos_v != old_frame_pos_v {
            request_sender.lock().unwrap().send(frame_pos_v as u64).unwrap()
        }
    });

    rsx! {
        <stack>
            <aspect_ratio aspect_ratio={4. / 3.}>
                {frame_with_number.map(|(number, frame)| rsx!{ <image image=frame.0 />})}
            </aspect_ratio>
            <align alignment=Alignment::bottom_center()>
                <padding>
                    <slider
                        val=(frame_number as f32)
                        on_change={move |context, new_val| {
                            context.shout(frame_pos, new_val as u64);
                        }}
                        min=0.0 max=(frame_count as f32 - 1.0)
                    />
                </padding>
            </align>
        </stack>
    }
}

fn main() {
    app::render(
        app::WindowBuilder::new().with_title("axiom raw player"),
        rsx_toplevel! {
            <player />
        },
    );
}
