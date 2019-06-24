use crate::{
    graphical::ui_lib::{Cache, DrawParams, Drawable, ShaderBox, SpatialProperties, Vec2},
    video_io::Image,
};
use glium::{
    backend::glutin::headless::Headless,
    texture::{self, MipmapsOption, Texture2d, UncompressedFloatFormat},
    uniform,
    DrawError,
};
use glutin::{ContextBuilder, EventsLoop};
use std::{borrow::Cow, collections::btree_map::BTreeMap, error, result::Result::Ok};

use glium::texture::RawImage2d;
use glutin::dpi::PhysicalSize;

pub trait Debayer {
    fn debayer(&self) -> Result<RawImage2d<u8>, Box<dyn error::Error>>;
}

impl Debayer for Image {
    fn debayer(&self) -> Result<RawImage2d<u8>, Box<dyn error::Error>> {
        let context = ContextBuilder::new()
            .build_headless(&EventsLoop::new(), PhysicalSize::new(1.0, 1.0))?;
        let facade = &mut Headless::new(context)?;
        let cache = &mut Cache(BTreeMap::new());

        let target_texture: Texture2d = Texture2d::empty_with_format(
            facade,
            UncompressedFloatFormat::U8U8U8U8,
            MipmapsOption::NoMipmap,
            self.width / 2,
            self.height / 2,
        )
        .unwrap();

        let source_texture = Texture2d::new(
            facade,
            texture::RawImage2d {
                data: Cow::from(self.data.clone()),
                width: self.width,
                height: self.height,
                format: texture::ClientFormat::U8,
            },
        )
        .unwrap();

        ShaderBox {
            fragment_shader: include_str!("../shader/debayer.frag").to_string(),
            uniforms: uniform! {raw_image: &source_texture},
        }
        .draw(
            &mut DrawParams {
                surface: &mut target_texture.as_surface(),
                facade,
                cache,
                screen_size: Vec2 { x: self.width, y: self.height },
            },
            SpatialProperties::full(),
        )?;

        let texture_data_sink = target_texture.read();

        Ok(texture_data_sink)
    }
}
