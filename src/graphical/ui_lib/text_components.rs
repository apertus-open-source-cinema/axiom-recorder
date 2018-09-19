extern crate euclid;

use crate::graphical::ui_lib::*;
use glium::texture;
use std::borrow::Cow;
use std::error::Error;
use std::result::Result::Ok;

use euclid::{Point2D, Size2D};
use font_kit::canvas::{Canvas, Format, RasterizationOptions};
use font_kit::family_name::FamilyName;
use font_kit::hinting::HintingOptions;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;

/// Draws a single glyph. Do not use this Directly
pub struct Letter {
    pub chr: char,
}

impl Letter {
    fn get_bitmap(&self) -> Result<Canvas, Box<Error>> {
        let font = SystemSource::new()
            .select_best_match(&[FamilyName::SansSerif], &Properties::new())
            .unwrap()
            .load()
            .unwrap();
        let glyph_id = font.glyph_for_char('A').unwrap();
        let mut canvas = Canvas::new(&Size2D::new(32, 32), Format::A8);
        font.rasterize_glyph(
            &mut canvas,
            glyph_id,
            32.0,
            &Point2D::zero(),
            HintingOptions::None,
            RasterizationOptions::GrayscaleAa,
        ).unwrap();
        Ok(canvas)
    }
}

impl<T> Drawable<T> for Letter
where
    T: Surface,
{
    fn draw(&self, params: &mut DrawParams<T>, sp: SpacialProperties) -> DrawResult {
        let bitmap = self.get_bitmap().unwrap();
        let texture = texture::Texture2d::new(
            params.facade,
            texture::RawImage2d {
                data: Cow::from(bitmap.pixels.clone()),
                width: bitmap.size.width as u32,
                height: bitmap.size.height as u32,
                format: texture::ClientFormat::U8,
            },
        ).unwrap();
        Ok(())
    }
}
