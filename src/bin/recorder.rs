#![feature(type_ascription)]

use clap::{App, Arg};

use recorder::{
    graphical::{
        self,
        settings::{self, Settings},
    },
    util::{error::Res, options::OptionsStorage},
    video_io::source::{BufferedVideoSource, MetaVideoSource},
};
use std::{any::Any, collections::HashMap, f64::NAN};

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
        .arg(
            Arg::with_name("no-histogram").long("no-histogram").help(
                "disables the histogram calculation. potentially saves A LOT of cpu ressources",
            ),
        )
        .arg(Arg::with_name("width").short("w").long("width").takes_value(true).required(true))
        .arg(Arg::with_name("height").short("h").long("height").takes_value(true).required(true))
        .arg(Arg::with_name("fps").long("fps").takes_value(true))
        .arg(Arg::with_name("debayer-options").long("debayer-options").takes_value(true))
        .get_matches();

    let source_str = arguments.value_of("video_source").unwrap();
    let options = &OptionsStorage::from_args(
        arguments.clone(),
        vec!["width", "height", "fps", "debayer-options", "no-histogram"],
    );

    let res = work(String::from(source_str), options);

    match res {
        Ok(_) => println!("\nsuccessfully ended recording session :)"),
        Err(error) => eprintln!("\nAn error occured: {}", error),
    }
}

fn work(source: String, options: &OptionsStorage) -> Res<()> {
    let video_source = MetaVideoSource::from_uri(source, options)?;
    let buffered_vs = BufferedVideoSource::new(Box::new(video_source));

    let initial_settings = Settings {
        shutter_angle: 180.0,
        iso: 800.0,
        fps: match options.get_opt_parse("fps") {
            Ok(fps) => fps,
            Err(_) => NAN,
        },
        recording_format: settings::RecordingFormat::Raw8,
        grid: settings::Grid::NoGrid,
        draw_histogram: !options.is_present("no-histogram"),
    };

    let mut graphical_manager = graphical::Manager::new(
        buffered_vs.subscribe(),
        initial_settings,
        options.get_opt_or("debayer-options", "source_lin(); debayer_halfresolution()"),
    )?;
    graphical_manager.run_event_loop()?;
    Ok(())
}
