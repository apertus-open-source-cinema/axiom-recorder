use super::{
    basic_components::*,
    layout_components::{Size::*, *},
    *,
};
use glium::texture;
use std::borrow::Cow;
use std::error::Error;
use std::result::Result::Ok;

use euclid::{Point2D, Size2D};
use font_kit::canvas::{Canvas, Format, RasterizationOptions};
use font_kit::hinting::HintingOptions;
use font_kit::loaders::freetype::Font;
use std::sync::Arc;

/// Draws a single glyph. Do not use this Directly
pub struct Letter {
    pub chr: char,
    pub size: u32,
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
        let size = &Size2D::new(
            raster_bounds.size.width as u32,
            raster_bounds.size.height as u32,
        );
        let mut canvas = Canvas::new(size, Format::A8);

        font.rasterize_glyph(
            &mut canvas,
            glyph_id,
            height as f32,
            &Point2D::origin(),
            HintingOptions::None,
            RasterizationOptions::GrayscaleAa,
        ).unwrap();
        Ok((
            canvas,
            Vec2 {
                x: raster_bounds.origin.x,
                y: raster_bounds.origin.y,
            },
        ))
    }
}

impl<T> Drawable<T> for Letter
where
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, T>, sp: SpatialProperties) -> DrawResult {
        let (bitmap, offset) = self.get_bitmap(self.size).unwrap();
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
            child: &SizeContainer {
                anchor: Vec2::zero(),
                size: Vec2 {
                    x: Px(bitmap.size.width),
                    y: Px(bitmap.size.height),
                },
                child: &MonoTextureBox {
                    color: [1., 1., 1., 1.],
                    texture,
                } as &Drawable<T>,
            } as &Drawable<T>,
        }.draw(params, sp)?;
        Ok(())
    }
}

/// Draws a whole String at once.
pub struct Text {
    pub str: String,
    pub size: u32,
}

const LETTER_WIDTH: f64 = 0.6;
impl<'a, T> Drawable<T> for Text
where
    T: Surface + 'a,
{
    fn draw(&self, params: &mut DrawParams<'_, T>, sp: SpatialProperties) -> DrawResult {
        let len = self.str.len();
        let drawables_vec: Vec<_> = self
            .str
            .chars()
            .enumerate()
            .map(|(i, chr)| {
                (
                    Box::from(Letter {
                        chr,
                        size: self.size,
                    }) as Box<Drawable<T>>,
                    SpatialProperties {
                        start: Vec2 {
                            x: (1. / len as f64) * i as f64,
                            y: 0.,
                        },
                        size: Vec2 {
                            x: 1. / len as f64,
                            y: 1.,
                        },
                    },
                )
            }).collect();

        SizeContainer {
            anchor: Vec2 { x: 0.5, y: 0.5 },
            size: Vec2 {
                x: Px((len as f64 * self.size as f64 * LETTER_WIDTH) as u32),
                y: Px(self.size),
            },
            child: &drawables_vec as &dyn Drawable<T>,
        }.draw(params, sp)
    }
}
