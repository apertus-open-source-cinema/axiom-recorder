use clap::{App, Arg};

use bus::Bus;
use indicatif::{ProgressBar, ProgressStyle};
use recorder::video_io::{
    source::{self, BufferedVideoSource, MetaVideoSource, VideoSource},
    writer::{MetaWriter, Writer},
};
use std::{
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};

fn main() {
    let arguments = App::new("Raw Image / Video Converter")
        .version("0.1")
        .about("convert raw footage from AXIOM cameras into other formats.")
        .arg(
            Arg::with_name("input")
                .short("i")
                .long("input")
                .takes_value(true)
                .required(true)
                .help("the path of the input video / image"),
        )
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output")
                .takes_value(true)
                .required(true)
                .help("the path of the output video / image"),
        )
        .arg(
            Arg::with_name("input-format")
                .long("input-format")
                .short("f")
                .allow_hyphen_values(true)
                .case_insensitive(true),
        )
        .arg(Arg::with_name("width").short("w").long("width").takes_value(true).required(true))
        .arg(Arg::with_name("height").short("h").long("height").takes_value(true).required(true))
        .arg(Arg::with_name("fps").long("fps").takes_value(true).required(true))
        .get_matches();

    let source_str = arguments.value_of("input").unwrap();
    let sink_str = arguments.value_of("output").unwrap();

    let height = arguments.value_of("height").unwrap().parse().unwrap();
    let width = arguments.value_of("width").unwrap().parse().unwrap();
    let fps = arguments.value_of("fps").unwrap().parse().unwrap();

    println!("\nconverting {} to {} ...\n", source_str, sink_str);

    {
        // connect source and sink
        let video_source =
            MetaVideoSource::from_file(String::from(source_str), width, height, None).unwrap();

        let mut sink = MetaWriter::new(String::from(sink_str), (width, height), fps).unwrap();

        let progressbar = match video_source.get_frame_count() {
            Some(n) => ProgressBar::new(n as u64),
            None => ProgressBar::new_spinner(),
        };

        progressbar.set_style(ProgressStyle::default_bar()
        .template("| {wide_bar} | {pos}/{len} frames | elapsed: {elapsed_precise} | remaining: {eta} |")
        .progress_chars("#>-"));

        video_source.get_images(&mut |frame| {
            progressbar.tick();
            progressbar.inc(1);

            sink.write_frame(Arc::new(frame));
        });
        progressbar.finish();
    }

    println!("\nsuccessfully converted {} to {} :)", source_str, sink_str)
}
