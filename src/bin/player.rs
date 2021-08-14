use narui::*;
use recorder::{
    frame::rgb_frame::RgbFrame,
    gui::image::*,
    pipeline_processing::{
        create_node_from_name,
        execute::execute_pipeline,
        parametrizable::{ParameterValue, Parameters},
        payload::Payload,
        processing_node::ProcessingNode,
    },
};
use std::{
    array::IntoIter,
    collections::HashMap,
    iter::FromIterator,
    sync::{
        mpsc::{sync_channel, Receiver},
        Arc,
        Mutex,
        MutexGuard,
    },
    thread::spawn,
};
use winit::{platform::unix::WindowBuilderExtUnix, window::WindowBuilder};

struct PlayerSink<T: Fn(Arc<RgbFrame>) + Send + Sync> {
    callback: T,
    fps_report: Mutex<FPSReporter>,
}
impl<T: Fn(Arc<RgbFrame>) + Send + Sync> ProcessingNode for PlayerSink<T> {
    fn process(
        &self,
        input: &mut Payload,
        _frame_lock: MutexGuard<u64>,
    ) -> anyhow::Result<Option<Payload>> {
        self.fps_report.lock().unwrap().frame();
        let frame = input.downcast::<RgbFrame>().expect("Wrong input format");
        (self.callback)(frame);
        Ok(Some(Payload::empty()))
    }
}

fn listenable_from_channel_handle<T: Send + Sync + PartialEq + 'static>(
    context: &Context,
    channel_handle: EffectHandle<Arc<Mutex<Receiver<T>>>>,
) -> Listenable<Option<T>> {
    // this dummy listenable gets updated to ensure that we get called again, even
    // if no new frame was available
    let dummy_listenable = context.listenable(0);
    context.listen(dummy_listenable);

    let current_frame = context.listenable(None);

    let rx = channel_handle.read().clone();
    context.after_frame(move |context: Context| {
        let lock = rx.try_lock().unwrap();
        if let Ok(frame) = lock.try_recv() {
            context.shout(current_frame, Some(frame));
        } else {
            let old = context.listen(dummy_listenable);
            context.shout(dummy_listenable, old + 1);
        }
    });

    current_frame
}


#[widget]
pub fn player(context: Context) -> Fragment {
    let handle = context.effect(move || {
        let (sender, receiver) = sync_channel(4);

        // TODO: handle thread destroy
        spawn(move || {
            let callback = move |frame: Arc<RgbFrame>| {
                sender.send(frame).unwrap();
            };

            let nodes = vec![
                create_node_from_name("RawDirectoryReader", &Parameters(HashMap::<_, _>::from_iter(IntoIter::new([
                    ("fps".to_string(), ParameterValue::FloatRange(24.0)),
                    ("file-pattern".to_string(), ParameterValue::StringParameter("/home/anuejn/code/apertus/axiom-recorder/test/Darkbox-Timelapse-Clock-Sequence/*".to_string())),
                    ("first-red-x".to_string(), ParameterValue::BoolParameter(false)),
                    ("first-red-y".to_string(), ParameterValue::BoolParameter(false)),
                    ("bit-depth".to_string(), ParameterValue::IntRange(12)),
                    ("width".to_string(), ParameterValue::IntRange(4096)),
                    ("height".to_string(), ParameterValue::IntRange(3072)),
                    ("loop".to_string(), ParameterValue::BoolParameter(true)),
                    ("sleep".to_string(), ParameterValue::FloatRange(1.0 / 24.0)),
                ])))).unwrap(),
                create_node_from_name("GpuBitDepthConverter", &Parameters(HashMap::new())).unwrap(),
                create_node_from_name("Debayer", &Parameters(HashMap::new())).unwrap(),
                Arc::new(PlayerSink { callback, fps_report: Mutex::new(FPSReporter::new("pipeline")) }) as Arc<dyn ProcessingNode>
            ];
            execute_pipeline(nodes).unwrap();
        });

        Arc::new(Mutex::new(receiver))
    }, ());

    let current_frame = listenable_from_channel_handle(&context, handle);

    let frame = context.listen(current_frame);
    if let Some(frame) = frame {
        rsx! { <image image={frame}/> }
    } else {
        rsx! {}
    }
}

fn main() {
    let window_builder = WindowBuilder::new()
        .with_title("ara player")
        .with_gtk_theme_variant("dark".parse().unwrap());

    render(
        window_builder,
        rsx_toplevel! {
            <player />
        },
    );
}
