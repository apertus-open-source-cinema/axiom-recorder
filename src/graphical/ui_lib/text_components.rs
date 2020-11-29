use super::{
    basic_components::*,
    container_components::*,
    layout_components::{Size::*, *},
    *,
};
use glium::texture;
use std::{borrow::Cow, error::Error, result::Result::Ok};

use crate::util::error::ResN;
use euclid::{Point2D, Size2D};
use font_kit::{
    canvas::{Canvas, Format, RasterizationOptions},
    hinting::HintingOptions,
    loaders::default::Font,
};
use pathfinder_geometry::transform2d::Transform2F;
use std::sync::Arc;

/// Draws a single glyph. Do not use this Directly
pub struct Letter {
    pub chr: char,
    pub size: u32,
    pub color: [f32; 4],
}

impl Letter {
    fn get_bitmap(&self, height: u32) -> Result<(Canvas, Vec2<i32>), Box<dyn Error>> {
        let font = Font::from_bytes(
            Arc::new(include_bytes!("../../../res/fonts/DejaVuSansMono.ttf").to_vec()),
            0,
        )
        .unwrap();
        let glyph_id = font.glyph_for_char(self.chr).unwrap();
        let raster_bounds = font.raster_bounds(
            glyph_id,
            height as f32,
            Transform2F::default(),
            HintingOptions::None,
            RasterizationOptions::GrayscaleAa,
        )?;
        let mut canvas = Canvas::new(raster_bounds.size(), Format::A8);
        font.rasterize_glyph(
            &mut canvas,
            glyph_id,
            height as f32,
            Transform2F::default(),
            HintingOptions::None,
            RasterizationOptions::GrayscaleAa,
        )?;
        Ok((canvas, Vec2 { x: raster_bounds.origin_x(), y: raster_bounds.origin_y() }))
    }
}

impl<S> Drawable<S> for Letter
where
    S: Surface + 'static,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> ResN {
        let (bitmap, offset) = self.get_bitmap(self.size)?;
        let texture = texture::Texture2d::new(
            params.facade,
            texture::RawImage2d {
                data: Cow::from(bitmap.pixels.clone()),
                width: bitmap.size.x() as u32,
                height: bitmap.size.y() as u32,
                format: texture::ClientFormat::U8,
            },
        )?;

        SizeContainer {
            anchor: Vec2::one(),
            size: Vec2 {
                x: Px((self.size as i32 - offset.x) as u32),
                y: Px((self.size as i32 - offset.y) as u32),
            },
            child: &(SizeContainer {
                anchor: Vec2::zero(),
                size: Vec2 { x: Px(bitmap.size.x() as u32), y: Px(bitmap.size.y() as u32) },
                child: &MonoTextureBox { color: self.color, texture },
            }),
        }
        .draw(params, sp)?;
        Ok(())
    }
}

/// Draws a whole String at once.
pub struct Text {
    pub str: String,
    pub size: u32,
    pub color: [f32; 4],
}

const LETTER_WIDTH: f64 = 0.6;

impl<S> Drawable<S> for Text
where
    S: Surface + 'static,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> ResN {
        let len = self.str.len();
        let letters: Vec<_> = self
            .str
            .chars()
            .map(|chr| {
                Box::from(Letter { chr, size: self.size, color: self.color })
                    as Box<dyn Drawable<_>>
            })
            .collect();
        let drawable_container =
            EqualDistributingContainer::Horizontal(letters as Vec<Box<dyn Drawable<_>>>);

        SizeContainer {
            anchor: Vec2 { x: 0.5, y: 0.5 },
            size: Vec2 {
                x: Px((len as f64 * self.size as f64 * LETTER_WIDTH) as u32),
                y: Px(self.size),
            },
            child: &drawable_container,
        }
        .draw(params, sp)
    }
}
