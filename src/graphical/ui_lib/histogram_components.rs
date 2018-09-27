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
};
use std::result::Result::Ok;

pub struct Histogram<'a> {
    pub raw_image: &'a Image,
    pub bins: u32,
    pub avrg: u32,
}

impl<'a> Histogram<'a> {
    pub fn generate_histogram(
        &self,
        context: &mut dyn Facade,
        cache: &mut Cache,
    ) -> Result<texture::Texture2d, DrawError> {
        let target_texture = texture::Texture2d::empty_with_format(
            context,
            UncompressedFloatFormat::F32,
            MipmapsOption::NoMipmap,
            self.bins,
            1,
        ).unwrap();


        #[derive(Copy, Clone)]
        struct SenselVertex {
            value: f32,
        }
        implement_vertex!(SenselVertex, value);

        let mut value_iter = (&self.raw_image.data).into_iter();
        let mut vertices = Vec::with_capacity((&self.raw_image.data).len() / self.avrg as usize);
        'outer: loop {
            let mut sum = 0;
            for _ in 0..self.avrg {
                match value_iter.next() {
                    Some(val) => sum += *val as u32,
                    None => break 'outer,
                }
            }
            vertices.push(SenselVertex { value: sum as f32 / self.avrg as f32 });
        }

        let vertex_shader = r#"
            #version 140
            in float value;

            void main() {
                float glcoord_pos = (value / 255.) * 2. - 1.;
                gl_Position = vec4(glcoord_pos, 0.0, 0.0, 1.0);
            }
        "#;

        let fragment_shader = r#"
            #version 140
            out vec4 color;

            void main() {
                color = vec4(1., 1., 1., 1. / 100000.);
            }
        "#;

        let program =
            glium::Program::from_source(context, vertex_shader, fragment_shader, None).unwrap();
        let vertex_buffer = glium::VertexBuffer::new(context, &vertices).unwrap();
        let mut target = target_texture.as_surface();
        target.clear_color(0.0, 0.0, 0.0, 0.0);

        target.draw(
            &vertex_buffer,
            glium::index::NoIndices(glium::index::PrimitiveType::Points),
            &program,
            &glium::uniforms::EmptyUniforms,
            &glium::DrawParameters { blend: Blend::alpha_blending(), ..Default::default() },
        )?;


        Ok(target_texture)
    }
}

impl<'a, S> Drawable<S> for Histogram<'a>
    where
        S: Surface,
{
    fn draw(&self, params: &mut DrawParams<'_, S>, sp: SpatialProperties) -> DrawResult {
        let histogram_data = self.generate_histogram(params.facade, params.cache)?;
        ShaderBox {
            fragment_shader: r#"
                #version 450
                uniform sampler2D in_image;
                in vec2 position;
                out vec4 color;

                void main(void) {
                    ivec2 size = textureSize(in_image, 0);
                    ivec2 pos = ivec2(size * position);
                    float f = texelFetch(in_image, ivec2(pos.x, 0), 0).r;
                    color = f > position.y ? vec4(1) : vec4(0);
                }
            "#.to_string(),
            uniforms: uniform! {
                in_image: &histogram_data,
            },
        }.draw(params, sp)
    }
}
