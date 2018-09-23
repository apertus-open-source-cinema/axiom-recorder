#![feature(specialization)]
use clap::{App, Arg};

mod graphical;
mod video_io;

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
        ).arg(
            Arg::with_name("width")
                .short("w")
                .long("width")
                .takes_value(true)
                .required(true),
        ).arg(
            Arg::with_name("height")
                .short("h")
                .long("height")
                .takes_value(true)
                .required(true),
        ).get_matches();

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
                Result::Ok(Box::new(video_io::source::FileVideoSource {
                    path: (*parts.get(1).unwrap()).to_string(),
                    height,
                    width,
                }))
            }
            _ => Result::Err(()),
        };
    let video_source = video_io::source::BufferedVideoSource::new(unbuffered_video_source.unwrap());

    let mut graphical_manager = graphical::Manager::new(video_source.subscribe());
    graphical_manager.run_event_loop();
}
