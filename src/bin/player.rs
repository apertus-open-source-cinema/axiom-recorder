use narui::*;
use std::{
    sync::mpsc::RecvTimeoutError,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use winit::{platform::unix::WindowBuilderExtUnix, window::WindowBuilder};

#[derive(Clone)]
enum Message {
    Stop,
}

#[widget]
pub fn player(context: Context) -> Fragment {
    let time = context.listenable("".to_string());
    context.thread(
        move |context, rx| loop {
            let now = SystemTime::now();
            let time_string = format!("{}", now.duration_since(UNIX_EPOCH).unwrap().as_secs());
            context.shout(time, time_string);
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(Message::Stop) => return,
                Err(RecvTimeoutError::Timeout) => {}
                _ => panic!(),
            }
        },
        Message::Stop,
        (),
    );

    rsx! {
         <text>{context.listen(time)}</text>
    }
}

fn main() {
    let window_builder = WindowBuilder::new()
        .with_title("narui clock demo")
        .with_gtk_theme_variant("dark".parse().unwrap());

    render(
        window_builder,
        rsx_toplevel! {
            <player />
        },
    );
}
