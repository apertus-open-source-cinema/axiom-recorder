use self::{
    settings::Settings,
    ui_lib::{
        basic_components::*,
        container_components::*,
        debayer_component::*,
        histogram_components::*,
        layout_components::{Size::*, *},
        text_components::*,
        *,
    },
};
use bus::BusReader;
use crate::video_io::{Image, source::BufferedVideoSource, writer::{Writer, CinemaDngWriter}};
use glium::{
    glutin::{ContextBuilder, EventsLoop, WindowBuilder},
    *,
};
use std::{
    collections::BTreeMap,
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};

mod settings;
mod ui_lib;

/// Manage the rendering process and orchestrate the rendering passes
pub struct Manager {
    display: Display,
    raw_image_source: BufferedVideoSource,
    event_loop: EventsLoop,
}

impl Manager {
    pub fn new(raw_image_source: BufferedVideoSource) -> Self {
        let event_loop = EventsLoop::new();
        let window = WindowBuilder::new();
        let context = ContextBuilder::new();
        let display = Display::new(window, context, &event_loop).unwrap();

        Manager { display, raw_image_source, event_loop }
    }

    pub fn run_event_loop(&mut self) {
        let cache = &mut Cache(BTreeMap::new());

        let mut closed = false;
        let mut last_image: Option<Arc<Image>> = None;
        let mut current_writer: Option<CinemaDngWriter> = None;
        let mut recording_since = None;
        let mut preview_feed = self.raw_image_source.subscribe();

        let mut should_reconsider_record = false;

        while !closed {
            let now = Instant::now();
            // listing the events produced by application and waiting to be received
            self.event_loop.poll_events(|ev| match ev {
                glutin::Event::WindowEvent { event, .. } => match event {
                    glutin::WindowEvent::CloseRequested => closed = true,
                    glutin::WindowEvent::KeyboardInput {
                        device_id,
                        input,
                    } => {
                        if input.virtual_keycode == Some(glutin::VirtualKeyCode::R) {
                            if input.state == glutin::ElementState::Pressed {
                                should_reconsider_record = true;
                            }                         
                        }
                    }
                    _ => (),
                },
                _ => (),
            });

            if should_reconsider_record {
                should_reconsider_record = false;

                match &current_writer {
                    Some(w) => { 
                        w.stop();
                        recording_since = None;
                        current_writer = None;
                    },
                    None => {
                        recording_since = Some(std::time::SystemTime::now());

                        current_writer = Some(CinemaDngWriter::start(
                            self.raw_image_source.subscribe(),
                            format!("recording-{}",  recording_since.unwrap().duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap().as_secs())
                        ));
                    }
                }
            }



            // look, wether we should debayer a new image
            let gui_settings: Settings = Settings {
                shutter_angle: 270.0,
                iso: 800.0,
                fps: 24.0,
                recording_format: settings::RecordingFormat::Raw8,
                grid: settings::Grid::NoGrid,
            };

            let draw_result = match preview_feed.recv_timeout(Duration::from_millis(10)) {
                Result::Err(_) => match last_image.clone() {
                    None => Ok(()),
                    Some(image) => self.redraw(image, &gui_settings, cache, recording_since),
                },
                Result::Ok(image) => {
                    last_image = Some(image.clone());
                    self.redraw(image, &gui_settings, cache, recording_since)
                }
            };

            if draw_result.is_err() {
                println!("draw error occured: \n {:#?}", draw_result.err().unwrap());
            }

            println!("{} fps", 1000 / now.elapsed().subsec_millis());
        }
    }

    pub fn redraw(
        &mut self,
        raw_image: Arc<Image>,
        gui_state: &settings::Settings,
        cache: &mut Cache,
        recording_since: Option<std::time::SystemTime>,
    ) -> Result<(), Box<dyn Error>> {
        let screen_size = Vec2::from(self.display.get_framebuffer_dimensions());
        let mut target = self.display.draw();
        target.clear_color(0.0, 0.0, 0.0, 0.0);

        let recording_duration = recording_since.and_then(|s| std::time::SystemTime::now().duration_since(s).ok());

        let rec_str = match recording_duration {
            Some(d) => {
                let secs = d.as_secs();
                let millis = d.subsec_millis();

                let hours = secs / 3600;
                let minutes = secs / 60;
                let secs = secs % 60;

                
                format!("{:02}:{:02}:{:02}:{:03}", hours, minutes, secs, millis)
            },
            None => "00:00:00:000".to_owned(),

        };

        let rec_dot = match recording_duration {
            Some(d) => "●".to_owned(),
            None => "".to_owned()
        };

        let draw_result = (vec![
            // the debayered image
            &AspectRatioContainer {
                aspect_ratio: raw_image.width as f64 / raw_image.height as f64,
                child: &Debayer { raw_image: &raw_image },
            },
            // the top bar
            &SizeContainer {
                anchor: Vec2 { x: 0., y: 1. },
                size: Vec2 { x: Percent(1.0), y: Px(50) },
                child: &vec![
                    &ColorBox { color: [0.0, 0.0, 0.0, 0.5] },
                    &SizeContainer {
                        anchor: Vec2::one(),
                        size: Vec2 { x: Percent(1.0), y: Px(42) },
                        child: &EqualDistributingContainer::Horizontal(
                            gui_state
                                .as_text()
                                .into_iter()
                                .map(|text| {
                                    Box::from(Text { str: text, size: 25, color: [1., 1., 1., 1.] })
                                        as Box<Drawable<_>>
                                }).collect(),
                        ),
                    },
                ]: &Vec<&Drawable<_>>,
            },
            // the bottom ba9
            &SizeContainer {
                anchor: Vec2 { x: 0., y: 0. },
                size: Vec2 { x: Percent(1.0), y: Px(80) },
                child: &vec![
                    /*
                    &SizeContainer {
                        anchor: Vec2 { x: 0., y: 0. },
                        size: Vec2 { x: Px(600), y: Px(80) },
                        child: &Histogram { raw_image: &raw_image },
                    },
                    */
                    &SizeContainer {
                        anchor: Vec2 { x: 1., y: 0. },
                        size: Vec2 { x: Px(300), y: Px(80) },
                        child: &Text {
                            str: rec_str,
                            size: 25,
                            color: [1., 1., 1., 1.],
                        },
                    },
                    &SizeContainer {
                        anchor: Vec2 { x: 1., y: 0. },
                        size: Vec2 { x: Px(300 * 2 - 50), y: Px(89) },
                        child: &Text { str: rec_dot, size: 30, color: [1., 0., 0., 1.] },
                    },
                ]: &Vec<&Drawable<_>>,
            },
        ]: Vec<&Drawable<_>>)
            .draw(
                &mut DrawParams {
                    surface: &mut target,
                    facade: &mut self.display,
                    cache,
                    screen_size,
                },
                SpatialProperties::full(),
            );

        target.finish()?;
        draw_result?;

        Ok(())
    }
}
