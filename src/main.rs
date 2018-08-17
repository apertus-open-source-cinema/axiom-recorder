extern crate gtk;

use gtk::prelude::*;

mod preview;
mod video_source;

fn main() {
    if gtk::init().is_err() {
        println!("Failed to initialize GTK.");
        return;
    }

    let glade_src = include_str!("recorder.glade");
    let builder = gtk::Builder::new_from_string(glade_src);

    let window: gtk::Window = builder.get_object("main_window").unwrap();
    window.show_all();

    // show preferences dialog on button click
    let show_preferences: gtk::Button = builder.get_object("show_preferences").unwrap();
    let preferences_dialog: gtk::Dialog = builder.get_object("preferences_dialog").unwrap();
    show_preferences.connect_clicked(move |_| {
        preferences_dialog.show();
    });

    // hide the preferences dialog on ok button click
    let close_preferences: gtk::Button = builder.get_object("close_preferences").unwrap();
    let preferences_dialog: gtk::Dialog = builder.get_object("preferences_dialog").unwrap();
    close_preferences.connect_clicked(move |_| {
        preferences_dialog.hide();
    });

    // start the video Source
    let video_source = video_source::

    // start the opengl rendering thread
    let gl_canvas: gtk::GLArea = builder.get_object("gl_canvas").unwrap();
    let open_gl_handler = preview::OpenGlHandler {
        context: gl_canvas.get_context().unwrap(),
    };
    open_gl_handler.start();

    gtk::main();
}
