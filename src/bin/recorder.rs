use clap::{App, Arg};

use recorder::{
    graphical::{
        self,
        settings::{self, Settings},
    },
    video_io,
};

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
        .get_matches();

    let source = arguments.value_of("video_source").unwrap();
    let parts = source.split("://").collect::<Vec<_>>();

    let height = arguments.value_of("height").unwrap().parse().unwrap();
    let width = arguments.value_of("width").unwrap().parse().unwrap();

    let unbuffered_video_source: Result<Box<dyn video_io::source::VideoSource>, ()> =
        match *parts.get(0).unwrap() {
            "tcp" => Result::Ok(Box::new(video_io::source::EthernetVideoSource {
                url: (*parts.get(1).unwrap()).to_string(),
                height,
                width,
            })),
            "file" => {
                println!("{}", (*parts.get(1).unwrap()).to_string());
                Result::Ok(Box::new(video_io::source::Raw8BlobVideoSource {
                    path: (*parts.get(1).unwrap()).to_string(),
                    height,
                    width,
                }))
            }
            _ => Result::Err(()),
        };
    let video_source = video_io::source::BufferedVideoSource::new(unbuffered_video_source.unwrap());

    let initial_settings = Settings {
        shutter_angle: 270.0,
        iso: 800.0,
        fps: 24.0,
        recording_format: settings::RecordingFormat::Raw8,
        grid: settings::Grid::NoGrid,
        draw_histogram: !arguments.is_present("no-histogram"),
    };

    let mut graphical_manager = graphical::Manager::new(video_source.subscribe(), initial_settings);
    graphical_manager.run_event_loop();
}
