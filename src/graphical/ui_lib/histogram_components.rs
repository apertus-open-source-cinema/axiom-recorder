use super::*;

use crate::util::error::ResN;
use glium::{
    texture::{self, RawImage2d},
    uniform,
    uniforms::{MagnifySamplerFilter::Nearest, Sampler},
    Surface,
};


pub struct Histogram<'a> {
    pub image: &'a RawImage2d<'a, u8>,
}

impl<'a> Histogram<'a> {
    pub fn generate_histogram(&self) -> Vec<Vec<u8>> {
        let mut rgba_hist: Vec<Vec<u32>> = (0..3).map(|_| (0..256).map(|_| 0).collect()).collect();
        for i in 0..self.image.data.len() {
            if i % 4 == 3 {
                continue;
            };
            rgba_hist[i % 4][self.image.data[i] as usize] += 1;
        }

        rgba_hist
            .iter()
            .map(|channel| {
                let mut sorted = channel.clone();
                sorted.sort();
                let median_calc = sorted[sorted.len() / 2];
                let median = if median_calc == 0 { 1 } else { median_calc };
                let median_dist_sum: u32 = channel
                    .iter()
                    .map(|x| if *x < median { (median - x) as u32 } else { (x - median) as u32 })
                    .sum::<u32>();
                let median_dist_avg = median_dist_sum / (channel.len() as u32);

                channel
                    .iter()
                    .map(|x| {
                        let y: u32 = (*x as u32) / ((median as u32 + 8 * median_dist_avg) / 256);
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
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> ResN {
        let histogram_data = self.generate_histogram();
        let mut texture_data: Vec<u8> = (0..256 * 3).map(|_| 0).collect();
        for i in 0..3 {
            for j in 0..histogram_data[i].len() {
                texture_data[j * 3 + i] = histogram_data[i][j];
            }
        }

        let source_texture = texture::Texture2d::new(
            params.facade,
            texture::RawImage2d::from_raw_rgb(texture_data, (256, 1)),
        )
        .unwrap();

        let sampler = Sampler::new(&source_texture).magnify_filter(Nearest);

        ShaderBox {
            fragment_shader: r#"
                #version 330
                uniform sampler2D data;
                in vec2 position;
                out vec4 color;

                void main(void) {
                    float px_size = 1.0 / 256.0;

                    vec3 p_this = texture(data, vec2(position.x, 0)).rgb;
                    vec3 p_prev = texture(data, vec2(position.x - px_size, 0)).rgb;
                    vec3 p_next = texture(data, vec2(position.x + px_size, 0)).rgb;

                    float x = mod(position.x, px_size) / px_size;
                    //vec3 point = 2.0*p_prev*pow(x, 2.0)  −3.0*p_prev*x  +p_prev  −4.0*p_this*pow(x, 2.0)  +4.0*p_this*x  +2.0*p_next*pow(x,2.0)  −p_next*x;

                    vec3 point = p_this;
                    color = vec4(0);
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
