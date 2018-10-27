use super::{
    basic_components::*,
    container_components::*,
    layout_components::{Size::*, *},
    *,
};
use glium::texture;
use std::{borrow::Cow, error::Error, result::Result::Ok};

use euclid::{Point2D, Size2D};
use font_kit::{
    canvas::{Canvas, Format, RasterizationOptions},
    hinting::HintingOptions,
    loaders::freetype::Font,
};
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
        ).unwrap();
        let glyph_id = font.glyph_for_char(self.chr).unwrap();
        let raster_bounds = font
            .raster_bounds(
                glyph_id,
                height as f32,
                &Point2D::origin(),
                HintingOptions::None,
                RasterizationOptions::GrayscaleAa,
            ).unwrap();
        let size = &Size2D::new(raster_bounds.size.width as u32, raster_bounds.size.height as u32);
        let mut canvas = Canvas::new(size, Format::A8);

        font.rasterize_glyph(
            &mut canvas,
            glyph_id,
            height as f32,
            &Point2D::origin(),
            HintingOptions::None,
            RasterizationOptions::GrayscaleAa,
        ).unwrap();
        Ok((canvas, Vec2 { x: raster_bounds.origin.x, y: raster_bounds.origin.y }))
    }
}

impl<S> Drawable<S> for Letter
    where
        S: Surface + 'static,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> DrawResult {
        flame::start("letter_draw");
        let rc = {
            let mut cache = params.cache.borrow_mut();
            cache.memoize_evil("letter", &format!("{}.{}", self.chr, self.size), || self.get_bitmap(self.size).unwrap())
        };

        let bitmap = &rc.0;
        let offset = &rc.1;

        let texture = texture::Texture2d::new(
            params.facade,
            texture::RawImage2d {
                data: Cow::from(bitmap.pixels.clone()),
                width: bitmap.size.width as u32,
                height: bitmap.size.height as u32,
                format: texture::ClientFormat::U8,
            },
        ).unwrap();

        SizeContainer {
            anchor: Vec2::one(),
            size: Vec2 {
                x: Px((self.size as i32 - offset.x) as u32),
                y: Px((self.size as i32 - offset.y) as u32),
            },
            child: &(SizeContainer {
                anchor: Vec2::zero(),
                size: Vec2 { x: Px(texture.width()), y: Px(texture.height()) },
                child: &MonoTextureBox { color: self.color, texture: &texture },
            }),
        }.draw(&mut DrawParams {
            surface: params.surface,
            facade: params.facade,
            cache: params.cache.clone(),
            screen_size: params.screen_size.clone(),
        }, sp)?;
        flame::end("letter_draw");
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
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> DrawResult {
        flame::start("text_draw");
        let len = self.str.len();
        let letters = self
            .str
            .chars()
            .map(|chr| {
                Box::from(Letter { chr, size: self.size, color: self.color }) as Box<Drawable<_>>
            }).collect(): Vec<_>;
        let drawable_container =
            EqualDistributingContainer::Horizontal(letters as Vec<Box<Drawable<_>>>);

        let ret = SizeContainer {
            anchor: Vec2 { x: 0.5, y: 0.5 },
            size: Vec2 {
                x: Px((len as f64 * self.size as f64 * LETTER_WIDTH) as u32),
                y: Px(self.size),
            },
            child: &drawable_container,
        }.draw(params, sp);
        flame::end("text_draw");
        ret
    }
}
