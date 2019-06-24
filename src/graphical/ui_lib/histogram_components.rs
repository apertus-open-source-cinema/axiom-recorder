use super::*;
use crate::video_io::Image;
use glium::{
    texture::{self, RawImage2d},
    uniform,
    uniforms::{MagnifySamplerFilter::Nearest, Sampler},
    Surface,
};
use std::borrow::Cow;

pub struct Histogram<'a> {
    pub image: &'a RawImage2d<'a, u8>,
}

impl<'a> Histogram<'a> {
    pub fn generate_histogram(&self) -> Vec<Vec<u8>> {
        let mut rgba_hist: Vec<Vec<u32>> = (0..4).map(|_| (0..256).map(|_| 0).collect()).collect();
        for i in 0..self.image.data.len() {
            rgba_hist[i % 4][self.image.data[i] as usize] += 1;
        }

        rgba_hist
            .iter_mut()
            .map(|channel| {
                (*channel).sort();
                let median_calc = channel[channel.len() / 2];
                let median = if median_calc == 0 { 1 } else { median_calc };
                let median_dist_sum: u32 = channel
                    .iter()
                    .map(|x| if *x < median { (median - x) as u32 } else { (x - median) as u32 })
                    .sum::<u32>();
                let median_dist_avg = median_dist_sum / (channel.len() as u32);

                channel
                    .iter()
                    .map(|x| {
                        let y: u32 = ((*x as u32) / ((median as u32 + 8 * median_dist_avg) / 256));
                        if y >= 255 {
                            255 as u8
                        } else {
                            y as u8
                        }
                    })
                    .collect()
            })
            .collect()
    }
}

impl<'a, S> Drawable<S> for Histogram<'a>
where
    S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> Res {
        let histogram_data = self.generate_histogram();
        let mut texture_data: Vec<u8> = (0..256*4).map(|_| 0).collect();
        for i in 0..4 {
            for j in 0..histogram_data[i].len() {
                texture_data[j * 4 + i] = histogram_data[i][j];
            }
        }

        let source_texture = texture::Texture2d::new(
            params.facade,
            texture::RawImage2d::from_raw_rgba(texture_data, (256, 1)),
        )
        .unwrap();

        let sampler = Sampler::new(&source_texture).magnify_filter(Nearest);

        ShaderBox {
            fragment_shader: r#"
                #version 450
                uniform sampler2D data;
                in vec2 position;
                out vec4 color;

                void main(void) {
                    vec3 point = texture(data, vec2(position.x, 0)).rgb;
                    vec4 color = vec4(1, 1, 1, 1);
                    if (point.r > position.y) {
                        color.r = 1.;
                    }
                    if (point.g > position.y) {
                        color.g = 1.;
                    }
                    if (point.b > position.y) {
                        color.b = 1.;
                    };
                    if (length(color) > 0.01) {
                        color.a = 1.;
                    }
                }
            "#
            .to_string(),
            uniforms: uniform! {
                data: sampler,
            },
        }
        .draw(params, sp)
    }
}
