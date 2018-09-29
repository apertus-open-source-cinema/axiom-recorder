use self::basic_components::TextureBox;
use super::*;
use crate::video_io::Image;
use glium::{
    backend::Facade,
    implement_vertex,
    texture::{self, MipmapsOption, UncompressedFloatFormat},
    uniform,
    DrawError,
    Surface,
    uniforms::{Sampler, MagnifySamplerFilter::Nearest},
};
use std::borrow::Cow;

pub struct Histogram<'a> {
    pub raw_image: &'a Image,
}

impl<'a> Histogram<'a> {
    pub fn generate_histogram(
        &self,
    ) -> Vec<u8> {
        let mut arr = [0 as u32; 256];
        for v in &self.raw_image.data {
            arr[*v as usize] += 1;
        }

        let max = arr.iter().max().unwrap();
        arr.iter().map(|x| (x / (max / 256)) as u8).collect()
    }
}

impl<'a, S> Drawable<S> for Histogram<'a>
    where
        S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> DrawResult {
        let histogram_data = self.generate_histogram();

        let source_texture = texture::Texture2d::new(
            params.facade,
            texture::RawImage2d {
                data: Cow::from(histogram_data),
                width: 256,
                height: 1,
                format: texture::ClientFormat::U8,
            },
        ).unwrap();

        let sampler = Sampler::new(&source_texture).magnify_filter(Nearest);

        ShaderBox {
            fragment_shader: r#"
                #version 450
                uniform sampler2D data;
                in vec2 position;
                out vec4 color;

                void main(void) {
                    float f = texture(data, vec2(position.x, 0)).r;
                    if (f > position.y) {
                        color = vec4(1);
                    } else {
                        color = vec4(0);
                    }
                }
            "#.to_string(),
            uniforms: uniform! {
                data: sampler,
            },
        }.draw(params, sp)
    }
}
