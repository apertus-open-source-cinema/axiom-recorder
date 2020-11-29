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
    *,
};
use std::{
    collections::BTreeMap,
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};
use glutin::event_loop::EventLoop;

pub mod settings;
pub mod ui_lib;

/// Manage the rendering process and orchestrate the rendering passes
pub struct Manager {
    display: Display,
    raw_image_source: BusReader<Arc<Image>>,
    event_loop: EventLoop<()>,
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
        let event_loop = glutin::event_loop::EventLoop::new();
        let wb = glutin::window::WindowBuilder::new().with_title("AXIOM recorder");
        let cb = glutin::ContextBuilder::new().with_vsync(true);
        let mut display = glium::Display::new(wb, cb, &event_loop).unwrap();

        let debayerer = Box::new(Debayerer::new(debayer_settings, size, &mut display)?);

        Ok(Manager { display, raw_image_source, event_loop, settings_gui, debayerer })
    }

    pub fn run_event_loop(mut self) -> Res<()> {
        let mut cache = Cache(BTreeMap::new());
        let mut last_image = Arc::new(Image { width: 1, height: 1, bit_depth: 1, data: vec![0] });
        let mut last_frame_time = Instant::now();

        let event_loop = self.event_loop;
        let mut raw_image_source = self.raw_image_source;
        let mut display = self.display;
        let mut debayerer = self.debayerer;
        event_loop.run(move |event, _, control_flow| {
            let next_frame_time = std::time::Instant::now();
            *control_flow = glutin::event_loop::ControlFlow::WaitUntil(next_frame_time);

            match event {
                glutin::event::Event::WindowEvent { event, .. } => match event {
                    glutin::event::WindowEvent::CloseRequested => {
                        *control_flow = glutin::event_loop::ControlFlow::Exit;
                        return;
                    },
                    _ => return,
                },
                glutin::event::Event::NewEvents(cause) => match cause {
                    glutin::event::StartCause::ResumeTimeReached { .. } => (),
                    glutin::event::StartCause::Init => (),
                    _ => return,
                },
                _ => return,
            }

            match raw_image_source.try_recv() {
                Result::Err(_) => {
                    println!("using last image again");
                    redraw(last_image.clone(), &mut cache, &mut display, debayerer.as_mut())
                },
                Result::Ok(image) => {
                    /*
                    loop {
                        // read all the frames that are stuck in the pipe to make the display non
                        // blocking
                        match self.raw_image_source.try_recv() {
                            Err(_) => break,
                            Ok(_) => (),
                        }
                    }
                    */

                    last_image = image.clone();
                    redraw(image, &mut cache, &mut display, debayerer.as_mut())
                }
            }.unwrap();

            let elapsed = last_frame_time.elapsed().subsec_millis();
            println!("frame");
            if elapsed > 0 {
                println!("{} fps (ui)", 1000 / elapsed);
                last_frame_time = Instant::now();
            }
        });
        Ok(())
    }
}


pub fn redraw(raw_image: Arc<Image>, cache: &mut Cache, display: &mut Display, debayerer: &mut Debayerer) -> Result<(), Box<dyn Error>> {
    let mut target = display.draw();
    target.clear_color_srgb(0.0, 0.0, 0.0, 1.0);

    let mut screen_size = Vec2::from(display.get_framebuffer_dimensions());

    /*
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
    */


    // let hist_component = Box::new(vec![]);


    let draw_result = (vec![
        // the debayered image
        &AspectRatioContainer {
            aspect_ratio: raw_image.width as f64 / raw_image.height as f64,
            //                child: &ImageComponent { image: &debayered },
            child: &TextureBox {
                texture: raw_image.debayer_drawable(debayerer, display)?,
            },
        } as &dyn Drawable<_>,
        /*

        // the top bar
        &SizeContainer {
            anchor: Vec2 { x: 0., y: 1. },
            size: Vec2 { x: Percent(1.0), y: Px(50) },
            child: &vec![
                &ColorBox { color: [0.0, 0.0, 0.0, 0.5] } as &dyn Drawable<_>,
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
            ],
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
                } as &dyn Drawable<_>,
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
            ],
        },
        */
    ])
        .draw(
            &mut DrawParams { surface: &mut target, facade: display, cache, screen_size },
            SpatialProperties::full(),
        );
    draw_result?;
    target.finish()?;
    Ok(())
}