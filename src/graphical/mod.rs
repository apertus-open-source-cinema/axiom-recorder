use self::{
    settings::Settings,
    ui_lib::{
        basic_components::*,
        container_components::*,
        histogram_components::*,
        layout_components::{Size::*, *},
        text_components::*,
        *,
    },
};
use crate::{
    debayer::{Debayer, Debayerer},
    util::{error::Res, image::Image},
};
use bus::BusReader;
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

pub mod settings;
pub mod ui_lib;

/// Manage the rendering process and orchestrate the rendering passes
pub struct Manager {
    display: Display,
    raw_image_source: BusReader<Arc<Image>>,
    event_loop: EventsLoop,
    settings_gui: Settings,
    debayerer: Box<Debayerer>,
}

impl Manager {
    pub fn new(
        raw_image_source: BusReader<Arc<Image>>,
        settings_gui: Settings,
        size: (u32, u32),
        debayer_settings: &str,
    ) -> Res<Self> {
        let event_loop = EventsLoop::new();
        let window = WindowBuilder::new();
        let context = ContextBuilder::new();
        let display = Display::new(window, context, &event_loop)?;

        let debayerer = Box::new(Debayerer::new(debayer_settings, size)?);

        Ok(Manager { display, raw_image_source, event_loop, settings_gui, debayerer })
    }

    pub fn run_event_loop(&mut self) -> Res<()> {
        let cache = &mut Cache::new();
        let deferrer = &mut Deferrer::new();

        let mut closed = false;
        let mut last_image = Arc::new(Image { width: 1, height: 1, bit_depth: 1, data: vec![0] });
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

            match self.raw_image_source.recv_timeout(Duration::from_millis(10)) {
                Result::Err(_) => self.redraw(last_image.clone(), cache, deferrer),
                Result::Ok(image) => {
                    loop {
                        // read all the frames that are stuck in the pipe to make the display non
                        // blocking
                        match self.raw_image_source.try_recv() {
                            Err(_) => break,
                            Ok(_) => (),
                        }
                    }
                    last_image = image.clone();
                    self.redraw(image, cache, deferrer)
                }
            }?;

            println!("{} fps (ui)", 1000 / now.elapsed().subsec_millis());
        }
        Ok(())
    }

    pub fn redraw(
        &mut self,
        raw_image: Arc<Image>,
        cache: &mut Cache,
        deferrer: &mut Deferrer,
    ) -> Result<(), Box<dyn Error>> {
        let screen_size = Vec2::from(self.display.get_framebuffer_dimensions());
        let mut target = self.display.draw();
        target.clear_color(0.0, 0.0, 0.0, 0.0);

        let debayered = match raw_image.debayer(self.debayerer.as_mut()) {
            Err(e) => {
                target.finish()?;
                return Err(e);
            }
            Ok(v) => v,
        };

        let hist_component: Box<dyn Drawable<_>> = if self.settings_gui.draw_histogram {
            Box::new(Histogram { image: &debayered })
        } else {
            Box::new(vec![])
        };

        let draw_result = (vec![
            // the debayered image
            &AspectRatioContainer {
                aspect_ratio: raw_image.width as f64 / raw_image.height as f64,
                child: &ImageComponent { image: &debayered },
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
                            self.settings_gui
                                .as_text()
                                .into_iter()
                                .map(|text| {
                                    Box::from(Text { str: text, size: 25, color: [1., 1., 1., 1.] })
                                        as Box<dyn Drawable<_>>
                                })
                                .collect(),
                        ),
                    },
                ]: &Vec<&dyn Drawable<_>>,
            },
            // the bottom bar
            &SizeContainer {
                anchor: Vec2 { x: 0., y: 0. },
                size: Vec2 { x: Percent(1.0), y: Px(80) },
                child: &vec![
                    &SizeContainer {
                        anchor: Vec2 { x: 0., y: 0. },
                        size: Vec2 { x: Px(600), y: Px(80) },
                        child: hist_component.as_ref(),
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
                ]: &Vec<&dyn Drawable<_>>,
            },
        ]: Vec<&dyn Drawable<_>>)
            .draw(
                &mut DrawParams {
                    surface: &mut target,
                    facade: &mut self.display,
                    cache,
                    deferrer: Some(deferrer),
                    screen_size,
                },
                SpatialProperties::full(),
            );

        target.finish()?;
        draw_result?;

        Ok(())
    }
}
