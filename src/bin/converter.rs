use clap::{App, Arg};

use bus::Bus;
use indicatif::{ProgressBar, ProgressStyle};
use recorder::video_io::{
    source::{self, BufferedVideoSource, VideoSource, VideoSourceHelper},
    writer::{PathWriter, Writer},
};
use std::sync::{Arc, Mutex};

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
                .short("if")
                .allow_hyphen_values(true)
                .case_insensitive(true),
        )
        .arg(Arg::with_name("width").short("w").long("width").takes_value(true).required(true))
        .arg(Arg::with_name("height").short("h").long("height").takes_value(true).required(true))
        .get_matches();

    let source_str = arguments.value_of("input").unwrap();
    let sink_str = arguments.value_of("output").unwrap();

    let height = arguments.value_of("height").unwrap().parse().unwrap();
    let width = arguments.value_of("width").unwrap().parse().unwrap();


    // connect source and sink
    let video_source =
        VideoSourceHelper::from_file(String::from(source_str), width, height, None).unwrap();
    let bus = Arc::new(Mutex::new(Bus::new(10)));
    let sink = PathWriter::from_path(bus.lock().unwrap().add_rx(), String::from(sink_str)).unwrap();

    let progressbar = match video_source.get_frame_count() {
        Some(n) => ProgressBar::new(n as u64),
        None => ProgressBar::new_spinner(),
    };

    progressbar.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .progress_chars("#>-"));

    video_source.get_images(&|frame| {
        progressbar.tick();
        progressbar.inc(1);
        bus.lock().unwrap().broadcast(Arc::new(frame));
    });

    progressbar.finish_with_message(
        format!("sucessfully converted {} to {}", source_str, sink_str).as_ref(),
    );
}
