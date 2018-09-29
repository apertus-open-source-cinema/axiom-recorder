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
use crate::video_io::Image;
use glium::{
    glutin::{ContextBuilder, EventsLoop, WindowBuilder},
    *,
};
use std::{
    collections::BTreeMap,
    error::Error,
    time::{Duration, Instant},
};

mod settings;
mod ui_lib;

/// Manage the rendering process and orchestrate the rendering passes
pub struct Manager {
    display: Display,
    raw_image_source: BusReader<Image>,
    event_loop: EventsLoop,
}

impl Manager {
    pub fn new(raw_image_source: BusReader<Image>) -> Self {
        let event_loop = EventsLoop::new();
        let window = WindowBuilder::new();
        let context = ContextBuilder::new();
        let display = Display::new(window, context, &event_loop).unwrap();

        Manager { display, raw_image_source, event_loop }
    }

    pub fn run_event_loop(&mut self) {
        let cache = &mut BTreeMap::new();

        let mut closed = false;
        let mut last_image: Option<Image> = None;
        while !closed {
            let now = Instant::now();
            // listing the events produced by application and waiting to be received
            self.event_loop.poll_events(|ev| match ev {
                glutin::Event::WindowEvent { event, .. } => match event {
                    glutin::WindowEvent::CloseRequested => closed = true,
                    _ => (),
                },
                _ => (),
            });

            // look, wether we should debayer a new image
            let gui_settings: Settings = Settings {
                shutter_angle: 270.0,
                iso: 800.0,
                fps: 24.0,
                recording_format: settings::RecordingFormat::Raw8,
                grid: settings::Grid::NoGrid,
            };

            let draw_result = match self.raw_image_source.recv_timeout(Duration::from_millis(10)) {
                Result::Err(_) => match last_image.clone() {
                    None => Ok(()),
                    Some(image) => self.redraw(image, &gui_settings, cache),
                },
                Result::Ok(image) => {
                    last_image = Some(image.clone());
                    self.redraw(image, &gui_settings, cache)
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
        raw_image: Image,
        gui_state: &settings::Settings,
        cache: &mut Cache,
    ) -> Result<(), Box<dyn Error>> {
        let screen_size = Vec2::from(self.display.get_framebuffer_dimensions());
        let mut target = self.display.draw();
        target.clear_color(0.0, 0.0, 0.0, 0.0);

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
                    &SizeContainer {
                        anchor: Vec2 { x: 0., y: 0. },
                        size: Vec2 { x: Px(600), y: Px(80) },
                        child: &Histogram {
                            raw_image: &raw_image,
                        },
                    },
                    &SizeContainer {
                        anchor: Vec2 { x: 1., y: 0. },
                        size: Vec2 { x: Px(300), y: Px(80) },
                        child: &Text {
                            str: "00:00:00:00".to_string(),
                            size: 25,
                            color: [1., 1., 1., 1.],
                        },
                    },
                    &SizeContainer {
                        anchor: Vec2 { x: 1., y: 0. },
                        size: Vec2 { x: Px(300 * 2 - 50), y: Px(89) },
                        child: &Text { str: "●".to_string(), size: 30, color: [1., 0., 0., 1.] },
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
