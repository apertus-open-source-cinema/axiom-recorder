use clap::{App, Arg};

use recorder::{
    graphical::{
        self,
        settings::{self, Settings},
    },
    video_io::source::{BufferedVideoSource, MetaVideoSource},
};
use std::f32::NAN;

fn main() {
    let arguments = App::new("AXIOM recorder")
        .version("0.1")
        .about("record raw footage from AXIOM cameras")
        .arg(
            Arg::with_name("video_source")
                .short("s")
                .long("video-source")
                .takes_value(true)
                .required(true)
                .help("a URI that describes the video source to use. Can be file:// or tcp://")
                .validator(|x| match x.split("://").count() {
                    2 => Result::Ok(()),
                    _ => Result::Err(String::from("invalid source URI format.")),
                }),
        )
        .arg(Arg::with_name("width").short("w").long("width").takes_value(true).required(true))
        .arg(Arg::with_name("height").short("h").long("height").takes_value(true).required(true))
        .arg(
            Arg::with_name("no-histogram").long("no-histogram").help(
                "disables the histogram calculation. potentially saves A LOT of cpu ressources",
            ),
        )
        .arg(Arg::with_name("fps").long("fps").takes_value(true))
        .get_matches();

    let source = arguments.value_of("video_source").unwrap();
    let height = arguments.value_of("height").unwrap().parse().unwrap();
    let width = arguments.value_of("width").unwrap().parse().unwrap();
    let fps = arguments.value_of("fps").map(|x| x.parse().unwrap());
    let video_source = MetaVideoSource::from_uri(String::from(source), width, height, fps).unwrap();
    let buffered_vs = BufferedVideoSource::new(Box::new(video_source));

    let initial_settings = Settings {
        shutter_angle: 180.0,
        iso: 800.0,
        fps: match fps {
            Some(fps) => fps,
            None => NAN,
        } as f64,
        recording_format: settings::RecordingFormat::Raw8,
        grid: settings::Grid::NoGrid,
        draw_histogram: !arguments.is_present("no-histogram"),
    };

    let mut graphical_manager = graphical::Manager::new(buffered_vs.subscribe(), initial_settings);
    graphical_manager.run_event_loop();
}
