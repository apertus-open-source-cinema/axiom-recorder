use narui::{app::render, *};
use recorder::{
    common::fps_report::FPSReporter,
    frame::rgb_frame::RgbFrame,
    gui::image::*,
    pipeline_processing::{
        create_node_from_name,
        execute::execute_pipeline,
        parametrizable::{ParameterValue, Parameters, VULKAN_CONTEXT},
        payload::Payload,
        processing_node::ProcessingNode,
    },
};
use std::{
    array::IntoIter,
    sync::{
        mpsc::{sync_channel, Receiver},
        Arc,
        Mutex,
        MutexGuard,
    },
    thread::spawn,
};
use winit::{dpi::PhysicalSize, platform::unix::WindowBuilderExtUnix, window::WindowBuilder};

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
    context: &mut WidgetContext,
    channel_handle: EffectHandle<Arc<Mutex<Receiver<T>>>>,
) -> Listenable<Option<T>> {
    // this dummy listenable gets updated to ensure that we get called again, even
    // if no new frame was available
    let dummy_listenable = context.listenable(0);
    context.listen(dummy_listenable);

    let current_frame = context.listenable(None);

    let rx = channel_handle.read().clone();
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
    let vk_context = Parameters(
        IntoIter::new([(
            VULKAN_CONTEXT.to_string(),
            ParameterValue::VulkanContext(
                context.vulkan_context.device.clone(),
                context.vulkan_context.queues.clone(),
            ),
        )])
        .collect(),
    );
    let handle = context.effect(
        move |_| {
            let (sender, receiver) = sync_channel(4);
            let vk_context = vk_context.clone();

            // TODO: handle thread destroy
            spawn(move || {
                let callback = move |frame: Arc<RgbFrame>| {
                    sender.send(frame).unwrap();
                };

                let nodes = vec![
                    create_node_from_name(
                        "RawDirectoryReader",
                        &Parameters(
                            IntoIter::new([
                                ("fps".to_string(), ParameterValue::FloatRange(24.0)),
                                (
                                    "file-pattern".to_string(),
                                    ParameterValue::StringParameter(
                                        "test/Darkbox-Timelapse-Clock-Sequence/*".to_string(),
                                    ),
                                ),
                                ("first-red-x".to_string(), ParameterValue::BoolParameter(false)),
                                ("first-red-y".to_string(), ParameterValue::BoolParameter(false)),
                                ("bit-depth".to_string(), ParameterValue::IntRange(12)),
                                ("width".to_string(), ParameterValue::IntRange(4096)),
                                ("height".to_string(), ParameterValue::IntRange(3072)),
                                ("loop".to_string(), ParameterValue::BoolParameter(true)),
                                ("sleep".to_string(), ParameterValue::FloatRange(1.0 / 24.0)),
                            ])
                            .collect(),
                        ),
                    )
                    .unwrap(),
                    create_node_from_name("GpuBitDepthConverter", &vk_context).unwrap(),
                    create_node_from_name("Debayer", &vk_context).unwrap(),
                    Arc::new(PlayerSink {
                        callback,
                        fps_report: Mutex::new(FPSReporter::new("pipeline")),
                    }) as Arc<dyn ProcessingNode>,
                ];
                execute_pipeline(nodes).unwrap();
            });

            Arc::new(Mutex::new(receiver))
        },
        (),
    );

    let current_frame = listenable_from_channel_handle(context, handle);
    let frame = context.listen(current_frame);

    let iso_idx = context.listenable(1usize);
    let deg_idx = context.listenable(1usize);

    rsx! {
        <stack>
            <backdrop_blur sigma=10.>
                <aspect_ratio aspect_ratio={4. / 3.}>
                    {frame.map(|frame| rsx! {<image image={frame} style={STYLE.fill()}/> })}
                </aspect_ratio>
            </backdrop_blur>
            <positioned>
                <padding padding=EdgeInsets::all(30.0)>
                    <column main_axis_alignment=MainAxisAlignment::SpaceBetween>
                        <rec_button>{"00:00:00".to_string()}</rec_button>
                        <row main_axis_alignment=MainAxisAlignment::SpaceBetween>
                            <sized constraint=BoxConstraints::tight_width(150.0)>
                                <rec_rect>
                                    <column>
                                        <text size=25.>{"1080p".to_string()}</text>
                                        <text size=25.>{"24 FPS".to_string()}</text>
                                    </column>
                                </rec_rect>
                            </sized>
                            <row main_axis_size=MainAxisSize::Min>
                                <rec_slide_button options=vec!["45°".to_string(), "90°".to_string(), "120°".to_string(), "180°".to_string(), "270°".to_string()] on_change={move |context: &CallbackContext, val: usize| {context.shout(deg_idx, val)}} val={context.listen(deg_idx)} />
                                <sized constraint=BoxConstraints::tight_width(50.0)>{None}</sized>
                                <rec_slide_button options=vec!["ISO 400".to_string(), "ISO 800".to_string(), "ISO 1600".to_string()] on_change={move |context: &CallbackContext, val: usize| {context.shout(iso_idx, val)}} val={context.listen(iso_idx)} />
                            </row>
                            <recording_button size=80. />
                        </row>
                    </column>
                </padding>
            </positioned>
        </stack>
    }
}

#[widget(on_change = (|_context, _new_val| {}))]
fn rec_slide_button(
    options: Vec<String>,
    val: usize,
    on_change: impl Fn(&CallbackContext, usize) + Send + Sync + Clone + 'static,
    context: &mut WidgetContext,
) -> Fragment {
    let on_change_clone = on_change.clone();
    let on_left = move |context: &CallbackContext, down: bool, _, _| {
        if down && val > 0 {
            on_change_clone(context, val - 1)
        }
    };
    let on_change_clone = on_change;
    let options_len = options.len();
    let on_right = move |context: &CallbackContext, down: bool, _, _| {
        if down && val < options_len - 1 {
            on_change_clone(context, val + 1)
        }
    };

    let text_size = 36.;

    let white = Color::new(1.0, 1.0, 1.0, 1.0);
    let transparent = Color::new(1., 1., 1., 0.1);

    rsx! {
        <rec_rect>
            <row main_axis_alignment=MainAxisAlignment::SpaceBetween>
                <input on_click=on_left>
                    <text size=text_size color={if val > 0 {white} else {transparent}}>{"<".to_string()}</text>
                </input>
                <text size=text_size >{options[val].clone()}</text>
                <input on_click=on_right>
                    <text size=text_size color={if val < options_len - 1 {white} else {transparent}}>{">".to_string()}</text>
                </input>
            </row>
        </rec_rect>
    }
}

#[widget]
pub fn rec_button(children: String, context: &mut WidgetContext) -> Fragment {
    rsx! {
        <rec_rect>
            <text size=36. >{children}</text>
        </rec_rect>
    }
}


#[widget]
pub fn rec_rect(children: Fragment, context: &mut WidgetContext) -> Fragment {
    rsx! {
        <rect
            constraint=BoxConstraints::tight(300., 70.)
            border_radius=Paxel(28.)
            stroke=Some((Color::new(1.0, 1.0, 1.0, 1.0), 2.0))
            fill=None
        >
            <padding padding=EdgeInsets::horizontal(20.0)>
                <align alignment=Alignment::center()>
                    {children}
                </align>
            </padding>
        </rect>
    }
}

#[widget(size = 80.)]
fn recording_button(size: f32, context: &mut WidgetContext) -> Fragment {
    rsx! {
        <rect
            border_radius=Fraction(1.)
            constraint=BoxConstraints::tight(size, size)
            stroke=Some((Color::new(1.0, 1.0, 1.0, 1.0), 5.0))
        >
            <padding padding=EdgeInsets::all(6.0)>
                <rect border_radius=Fraction(1.) fill=Some(Color::new(1.0, 0.2, 0.2, 1.0))/>
            </padding>
        </rect>
    }
}

fn main() {
    env_logger::init();
    let window_builder = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(1200, 900))
        .with_title("axiom recorder")
        .with_gtk_theme_variant("dark".parse().unwrap());

    render(
        window_builder,
        rsx_toplevel! {
            <player />
        },
    );
}
