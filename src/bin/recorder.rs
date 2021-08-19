use narui::*;
use narui::style::*;
use narui::Style;
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
        rsx! {
            <container style={STYLE.fill()}>
                <aspect_ratio_container style={STYLE.fill()} aspect_ratio={4. / 3.}>
                    <image image={frame} style={STYLE.fill()}/>
                </aspect_ratio_container>
                <container style={STYLE.position_type(Absolute).height(Points(100.)).width_fill().flex_direction(Column).justify_content(JustifyContent::Center).align_items(AlignItems::Center)}>
                    <rec_button>{"00:00:00".to_string()}</rec_button>
                </container>
                <rounded_rect
                    style={STYLE.position_type(Absolute).bottom(Points(0.)).height(Points(122.)).width_fill().justify_content(JustifyContent::SpaceAround).align_items(AlignItems::Center)}
                    fill_color=Some(Color::new(0.0, 0.0, 0.0, 0.8))
                >
                    <rec_slide_button options=vec!["ISO 400".to_string(), "ISO 800".to_string(), "ISO 1600".to_string()] val=1 />
                    <rec_slide_button options=vec!["ISO 400".to_string(), "ISO 800".to_string(), "ISO 1600".to_string()] val=1 />
                    <recording_button size=80. />
                </rounded_rect>
            </container>
        }
    } else {
        rsx! {}
    }
}

#[widget(style = Default::default(), on_change = (|_context, _new_val| {}))]
fn rec_slide_button(
    style: Style,
    options: Vec<String>,
    val: usize,
    on_change: impl Fn(Context, String) + Send + Sync + Clone + 'static,
    context: Context
) -> Fragment {
    let on_change_clone = on_change.clone();
    let on_left = move |context: Context, down: bool| {

    };
    let on_change_clone = on_change.clone();
    let on_right = move |context: Context, down: bool| {

    };

    let text_size = 36.;

    rsx! {
        <rec_rect>
            <input on_click=on_left>
                <text size=text_size >{"<".to_string()}</text>
            </input>
            <text size=text_size >{options[val].clone()}</text>
            <input on_click=on_right>
                <text size=text_size >{">".to_string()}</text>
            </input>
        </rec_rect>
    }
}

#[widget(style = Default::default())]
pub fn rec_button(style: Style, children: String, context: Context) -> Fragment {
    rsx! {
        <rec_rect style=style>
            <text size=36. >{children}</text>
        </rec_rect>
    }
}


#[widget(style = Default::default())]
pub fn rec_rect(style: Style, children: Vec<Fragment>, context: Context) -> Fragment {
    rsx! {
        <rounded_rect
            style=style.width(Points(300.)).height(Points(70.)).padding(Points(20.)).justify_content(JustifyContent::SpaceAround).align_items(AlignItems::Center)
            border_radius=Points(28.)
            stroke_color=Some(color!(#ffffff))
            fill_color=None
        >
            {children}
        </rounded_rect>
    }
}

#[widget(style = Default::default(), size = 80.)]
fn recording_button(style: Style, size: f32, context: Context) -> Fragment {
    rsx! {
        <rounded_rect
            border_radius=Percent(1.)
            style=style.width(Points(size)).height(Points(size)).padding(Points(6.))
            stroke_color=Some(color!(#ffffff))
            fill_color=None
            stroke_options={StrokeOptions::default().with_line_width(5.)}
        >
            <rounded_rect border_radius=Percent(1.) style=STYLE.fill() fill_color=Some(color!(#FF5C5C))/>
        </rounded_rect>
    }
}

#[widget(style = Default::default())]
fn aspect_ratio_container(children: Vec<Fragment>, aspect_ratio: f32, style: Style, context: Context) -> Fragment {
    rsx! {
        <container style={style.justify_content(JustifyContent::Center).flex_direction(Row)}>
            <container style={STYLE.justify_content(JustifyContent::Center).flex_direction(Column).aspect_ratio(Defined(aspect_ratio))}>
                <container style={STYLE.aspect_ratio(Defined(1. / aspect_ratio))}>  // TODO: this is a stretch bug
                    {children}
                </container>
            </container>
        </container>
    }
}

fn main() {
    let window_builder = WindowBuilder::new()
        .with_title("axiom recorder")
        .with_gtk_theme_variant("dark".parse().unwrap());

    render(
        window_builder,
        rsx_toplevel! {
            <player />
        },
    );
}
