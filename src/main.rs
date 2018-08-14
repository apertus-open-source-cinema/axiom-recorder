extern crate gtk;

use gtk::prelude::*;

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
    let preferences_dialog: gtk::MessageDialog = builder.get_object("preferences_dialog").unwrap();

    show_preferences.connect_clicked(move |_| {
        // We make the dialog window blocks all other windows.
        preferences_dialog.run();
        // When it finished running, we hide it again.
        preferences_dialog.hide();
    });




    gtk::main();
}