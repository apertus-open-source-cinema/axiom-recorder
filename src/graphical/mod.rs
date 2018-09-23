use self::settings::Settings;
use self::ui_lib::{
    basic_components::*,
    debayer_component::*,
    layout_components::{Size::*, *},
    text_components::*,
    *,
};
use bus::BusReader;
use crate::video_io::Image;
use glium::glutin::{ContextBuilder, EventsLoop, WindowBuilder};
use glium::*;
use std::collections::BTreeMap;
use std::error::Error;
use std::time::{Duration, Instant};

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

        Manager {
            display,
            raw_image_source,
            event_loop,
        }
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
                shutter_angle: 0.0,
                iso: 0.0,
                fps: 0.0,
                recording_format: settings::RecordingFormat::RawN,
                grid: settings::Grid::None,
            };

            let draw_result = match self
                .raw_image_source
                .recv_timeout(Duration::from_millis(10))
                {
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

        let draw_result = vec![
            // the debayered image
            &AspectRatioContainer {
                aspect_ratio: raw_image.width as f64 / raw_image.height as f64,
                child: &Debayer { raw_image },
            } as &dyn Drawable<Frame>,
            // the top bar
            &SizeContainer {
                anchor: Vec2 { x: 0., y: 1. },
                size: Vec2 {
                    x: Percent(1.0),
                    y: Px(50),
                },
                child: &vec![
                    &ColorBox {
                        color: [0.0, 0.0, 0.0, 0.5],
                    } as &dyn Drawable<Frame>,
                    &Text {
                        str: "ISO 800".to_string(),
                        size: 30,
                    } as &dyn Drawable<Frame>,
                ] as &dyn Drawable<Frame>,
            } as &dyn Drawable<Frame>,
            &Text {
                str: "Apertus° AXIOM recorder".to_string(),
                size: 40,
            } as &dyn Drawable<Frame>,
            // the bottom bar
            &SizeContainer {
                anchor: Vec2 { x: 0., y: 0. },
                size: Vec2 {
                    x: Percent(1.0),
                    y: Px(80),
                },
                child: &ColorBox {
                    color: [0.0, 0.0, 0.0, 0.5],
                } as &dyn Drawable<Frame>,
            } as &dyn Drawable<Frame>,
        ].draw(
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
