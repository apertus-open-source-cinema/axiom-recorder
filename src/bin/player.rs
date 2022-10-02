use futures::executor::block_on;
use narui::*;
use recorder::{
    gui::image::{image, ArcPartialEqHelper},
    pipeline_processing::{
        buffers::GpuBuffer,
        frame::{Frame, Rgb},
        node::NodeID,
        processing_context::ProcessingContext,
        processing_graph::{ProcessingGraph, ProcessingGraphBuilder, SerdeNodeConfig},
    },
};
use std::{
    sync::{
        mpsc::{sync_channel, Receiver},
        Arc,
        Mutex,
    },
    thread::spawn,
};


fn listenable_from_channel_handle<T: Send + Sync + PartialEq + 'static>(
    context: &mut WidgetContext,
    channel_handle: Arc<Mutex<Receiver<T>>>,
) -> Listenable<Option<T>> {
    // this dummy listenable gets updated to ensure that we get called again, even
    // if no new frame was available
    let dummy_listenable = context.listenable(0);
    context.listen(dummy_listenable);

    let current_frame = context.listenable(None);

    let rx = channel_handle.clone();
    context.after_frame(move |context: &CallbackContext| {
        let lock = rx.try_lock().unwrap();
        if let Ok(frame) = lock.try_recv() {
            context.shout(current_frame, Some(frame));
        } else {
            let old = context.spy(dummy_listenable);
            context.shout(dummy_listenable, old + 1);
        }
    });

    current_frame
}


#[widget]
pub fn player(context: &mut WidgetContext) -> Fragment {
    let vulkan_context = context.vulkan_context.clone();


    let handle = context.effect(
        move |_| {
            let processing_context = ProcessingContext::from_vk_device_queues(
                vulkan_context.device.clone(),
                vulkan_context.queues.clone(),
            );
            let build_graph = || -> anyhow::Result<(ProcessingGraph, NodeID)> {
                let mut graph_builder = ProcessingGraphBuilder::new();
                graph_builder.add(
                    "reader".to_string(),
                    serde_yaml::from_str::<SerdeNodeConfig>(
                        "
                      type: RawDirectoryReader
                      file-pattern: test/Darkbox-Timelapse-Clock-Sequence/*
                      width: 4096
                      height: 3072
                      rgb: false
                ",
                    )?
                    .into(),
                )?;
                graph_builder.add(
                    "converter".to_string(),
                    serde_yaml::from_str::<SerdeNodeConfig>(
                        "
                    type: GpuBitDepthConverter
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

            let (request_sender, request_receiver) = sync_channel::<u64>(3);
            let (response_sender, response_receiver) =
                sync_channel::<(u64, ArcPartialEqHelper<Frame<Rgb, GpuBuffer>>)>(3);
            request_sender.send(0).unwrap();
            spawn(move || {
                for i in request_receiver {
                    let image =
                        block_on(debayer.pull(i, NodeID::from(usize::MAX), &processing_context))
                            .unwrap();
                    response_sender
                        .send((i, ArcPartialEqHelper(image.downcast().unwrap())))
                        .unwrap();
                }
            });

            (caps, Arc::new(Mutex::new(response_receiver)), Arc::new(Mutex::new(request_sender)))
        },
        (),
    );

    let caps = handle.read().0;
    let frame_count = caps.frame_count.unwrap_or(1);
    let response_receiver = handle.read().1.clone();
    let request_sender = handle.read().2.clone();

    let frame_listenable = listenable_from_channel_handle(context, response_receiver.clone());
    let frame_with_number = context.listen(frame_listenable);
    let frame_number = frame_with_number.clone().map_or(0, |(number, _)| number);
    rsx! {
        <stack>
            <aspect_ratio aspect_ratio={4. / 3.}>
                {frame_with_number.map(|(number, frame)| rsx!{ <image image=frame.0 />})}
            </aspect_ratio>
            <align alignment=Alignment::bottom_center()>
                <padding>
                    <slider
                        val=(frame_number as f32)
                        on_change={move |context: &CallbackContext, new_val| {
                            request_sender.lock().unwrap().send(new_val as u64).unwrap()
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
