use clap::{App, Arg};


use indicatif::{ProgressBar, ProgressStyle};
use recorder::{
    util::{error::Res, options::OptionsStorage},
    video_io::{
        source::{MetaVideoSource, VideoSource},
        writer::{MetaWriter, Writer},
    },
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};

fn main() {
    let arguments = App::new("Raw Image / Video Converter")
        .version("0.1")
        .about("convert raw footage from AXIOM cameras into other formats.")
        // required arguments
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
        // options (are handled by some special mechanism)
        .arg(Arg::with_name("width").short("w").long("width").takes_value(true))
        .arg(Arg::with_name("height").short("h").long("height").takes_value(true))
        .arg(Arg::with_name("fps").long("fps").takes_value(true))
        .arg(Arg::with_name("debayer-options").long("debayer-options").takes_value(true))
        .get_matches();

    let source_str = arguments.value_of("input").unwrap();
    let sink_str = arguments.value_of("output").unwrap();

    let options = &OptionsStorage::from_args(
        arguments.clone(),
        vec!["width", "height", "fps", "debayer-options"],
    );

    println!("\nconverting {} to {} ...\n", source_str, sink_str);

    let res = work(source_str, sink_str, options);

    match res {
        Ok(_) => println!("\nsuccessfully converted {} to {} :)", source_str, sink_str),
        Err(error) => eprintln!("\nAn error occured: {}", error),
    }
}

// used to have the convenience of the ? macro for error handling
fn work(source_str: &str, sink_str: &str, options: &OptionsStorage) -> Res<()> {
    {
        // connect source and sink
        let source = MetaVideoSource::from_file(String::from(source_str), options)?;
        let mut sink = MetaWriter::new(String::from(sink_str), options)?;

        let progressbar = match source.get_frame_count() {
            Some(n) => ProgressBar::new(n as u64),
            None => ProgressBar::new_spinner(),
        };

        progressbar.set_style(ProgressStyle::default_bar()
        .template("| {wide_bar} | {pos}/{len} frames | elapsed: {elapsed_precise} | remaining: {eta} |")
        .progress_chars("#>-"));

        source.get_images(&mut |frame| {
            sink.write_frame(Arc::new(frame))?;
            progressbar.tick();
            progressbar.inc(1);
            Ok(())
        })?;
        progressbar.finish();

        Ok(())
    }
}
