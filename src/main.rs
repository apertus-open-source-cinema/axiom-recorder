extern crate gdk;
extern crate gtk;

use gtk::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};

mod preview;
mod video_source;
mod writer;

fn main() {
    if gtk::init().is_err() {
        println!("Failed to initialize GTK.");
        return;
    }

    let glade_src = include_str!("recorder.glade");
    let builder = gtk::Builder::new_from_string(glade_src);

    let window: gtk::Window = builder.get_object("main_window").unwrap();

    // show preferences dialog on button click
    let show_preferences: gtk::Button = builder.get_object("show_preferences").unwrap();
    let preferences_dialog: gtk::Dialog = builder.get_object("preferences_dialog").unwrap();
    show_preferences.connect_clicked(move |_| { preferences_dialog.show(); });

    // hide the preferences dialog on ok button click
    let close_preferences: gtk::Button = builder.get_object("close_preferences").unwrap();
    let preferences_dialog: gtk::Dialog = builder.get_object("preferences_dialog").unwrap();
    close_preferences.connect_clicked(move |_| { preferences_dialog.hide(); });

    // start the video Source

    /*
    let vs = video_source::FileVideoSource {
        path: "rec-1535159119.raw8".to_string(),
        width: 2304,
        height: 1296,
        bit_depth: 8,
    };
    */

    let vs = video_source::EthernetVideoSource {
        url: "axiom-micro:8080".to_string(),
        width: 2304,
        height: 1296,
        bit_depth: 8,
    };

    let video_source = video_source::BufferedVideoSource::new(vs);

    // start the opengl rendering thread
    let glarea: gtk::GLArea = builder.get_object("gl_canvas").unwrap();

    let opengl_handler = preview::OpenGlHandler::new(
        glarea,
        video_source.subscribe()
    );

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    /*
    let writer = writer::Writer::new(
        video_source.subscribe(),
        format!("rec-{}.raw8", now)
    );

    */
    window.set_default_size(2304, 1346);
    window.show_all();

    gtk::main();
}
