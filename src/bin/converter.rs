use clap::{App, Arg};

use recorder::video_io::{Image, source, writer};
use recorder::video_io::source::VideoSource;

fn main() {
    let arguments = App::new("Raw Image / Video Converter")
        .version("0.1")
        .about("convert raw footage from AXIOM cameras into othir formas")
        .arg(
            Arg::with_name("input")
                .short("i")
                .long("input")
                .takes_value(true)
                .required(true)
                .help("the path of the input video / image")
                .validator(|filename| match filename.ends_with(".raw8") {
                    true => Ok(()),
                    false => Err(String::from("Currently only raw8 input files are supported")),
                }),
        )
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output")
                .takes_value(true)
                .required(true)
                .help("the path of the output video / image"),
        )
        .arg(Arg::with_name("width").short("w").long("width").takes_value(true).required(true))
        .arg(Arg::with_name("height").short("h").long("height").takes_value(true).required(true))
        .get_matches();

    let source_str = arguments.value_of("input").unwrap();
    let sink_str = arguments.value_of("output").unwrap();

    let height = arguments.value_of("height").unwrap().parse().unwrap();
    let width = arguments.value_of("width").unwrap().parse().unwrap();

    let source = source::Raw8BlobVideoSource {
        path: source_str.to_string(),
        width,
        height,
    };

    source.get_images(&|img| {

    });
}