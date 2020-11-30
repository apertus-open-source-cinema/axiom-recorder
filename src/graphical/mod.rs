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
    debayer::{Debayer, OnscreenDebayerer},
    util::{error::Res, fps_report::FPSReporter, image::Image},
};
use bus::BusReader;
use glium::*;
use glutin::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};
use std::{
    collections::BTreeMap,
    error::Error,
    ops::Add,
    sync::Arc,
    time::{Duration, Instant},
};


pub mod settings;
pub mod ui_lib;

/// Manage the rendering process and orchestrate the rendering passes
pub struct Manager {
    display: Display,
    raw_image_source: BusReader<Arc<Image>>,
    event_loop: EventLoop<()>,
    settings_gui: Settings,
    debayerer: Box<OnscreenDebayerer>,
}

impl Manager {
    pub fn new(
        raw_image_source: BusReader<Arc<Image>>,
        settings_gui: Settings,
        size: (u32, u32),
        debayer_settings: &str,
    ) -> Res<Self> {
        let event_loop = glutin::event_loop::EventLoop::new();
        let window_builder = glutin::window::WindowBuilder::new().with_title("AXIOM recorder");
        let windowed_context = glutin::ContextBuilder::new()
            .with_vsync(true)
            .build_windowed(window_builder, &event_loop)?;
        let mut display = glium::Display::from_gl_window(windowed_context)?;

        let debayerer = Box::new(OnscreenDebayerer::new(debayer_settings, size, &mut display)?);

        Ok(Manager { display, raw_image_source, event_loop, settings_gui, debayerer })
    }

    pub fn run_event_loop(self) -> Res<()> {
        let mut cache = Cache(BTreeMap::new());
        let mut last_image = Arc::new(Image::new(1, 1, vec![0u8], 8)?);
        let mut last_frame_time = Instant::now();

        let event_loop = self.event_loop;
        let mut raw_image_source = self.raw_image_source;
        let mut display = self.display;
        let mut debayerer = self.debayerer;
        let mut screen_size = Vec2::from(display.get_framebuffer_dimensions());
        let mut fps_reporter = FPSReporter::new("ui");

        event_loop.run(move |event, _, control_flow| {
            *control_flow =
                ControlFlow::WaitUntil(last_frame_time.add(Duration::from_millis(1000 / 60)));
            let mut redraw_requested = false;
            match event {
                Event::LoopDestroyed => return,
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::Resized(physical_size) => {
                        display.gl_window().resize(physical_size);
                        screen_size = Vec2::from((physical_size.width, physical_size.height));
                    }
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    _ => (),
                },
                Event::RedrawRequested(_) => {
                    redraw_requested = true;
                }
                _ => (),
            }

            // somehow this code is a bit fragile; ask @anuejn before changing
            if (last_frame_time.elapsed().subsec_millis() > 1000 / 30) || redraw_requested {
                fps_reporter.frame();
                match raw_image_source.try_recv() {
                    Result::Err(_) => {
                        println!("using last image again");
                        redraw(
                            last_image.clone(),
                            &mut cache,
                            &mut display,
                            debayerer.as_mut(),
                            &screen_size,
                        )
                    }
                    Result::Ok(image) => {
                        loop {
                            // read all the frames that are stuck in the pipe to make the display
                            // non blocking
                            match raw_image_source.try_recv() {
                                Err(_) => break,
                                Ok(_) => (),
                            }
                        }

                        last_image = image.clone();
                        redraw(image, &mut cache, &mut display, debayerer.as_mut(), &screen_size)
                    }
                }
                .unwrap();
                last_frame_time = Instant::now();
            }
        });
    }
}


pub fn redraw(
    raw_image: Arc<Image>,
    cache: &mut Cache,
    display: &mut Display,
    debayerer: &mut OnscreenDebayerer,
    screen_size: &Vec2<u32>,
) -> Result<(), Box<dyn Error>> {
    let mut target = display.draw();
    target.clear_color_srgb(0.0, 0.0, 0.0, 1.0);

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
            child: &TextureBox { texture: raw_image.debayer_to_drawable(debayerer, display)? },
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
        &mut DrawParams {
            surface: &mut target,
            facade: display,
            cache,
            screen_size: screen_size.clone(),
        },
        SpatialProperties::full(),
    );
    draw_result?;
    target.finish()?;
    Ok(())
}
