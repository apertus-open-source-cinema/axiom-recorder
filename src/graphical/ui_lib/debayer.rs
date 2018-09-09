use video_io::Image;
use glium::backend::Facade;
use glium::texture::UncompressedFloatFormat;
use glium::texture::MipmapsOption;
use glium::texture;
use glium::Program;
use graphical::gl_util;
use glium::index;
use std::borrow::Cow;
use graphical::ui_lib::*;

pub struct Debayer<'a> {
    raw_image: Image,
    context: &'a Facade,
}

impl<'a> Debayer<'a> {
    pub fn debayer(raw_image: &Image, context: &Facade) -> texture::Texture2d {
        let target_texture = texture::Texture2d::empty_with_format(
            context,
            UncompressedFloatFormat::U8U8U8U8,
            MipmapsOption::NoMipmap,
            raw_image.width,
            raw_image.height,
        ).unwrap();

        let source_texture = texture::Texture2d::new(
            context,
            texture::RawImage2d {
                data: Cow::from(raw_image.data.clone()),
                width: raw_image.width,
                height: raw_image.height,
                format: texture::ClientFormat::U8,
            },
        ).unwrap();

        let program = Program::from_source(
            context,
            gl_util::PASSTHROUGH_VERTEX_SHADER_SRC,
            include_str!("debayer.frag"),
            None,
        ).unwrap();

        target_texture.as_surface().draw(
            &gl_util::Vertex::triangle_strip_surface(context, (-1.0, -1.0, 1.0, 1.0)),
            &index::NoIndices(index::PrimitiveType::TriangleStrip),
            &program,
            &uniform! {raw_image: &source_texture},
            &Default::default(),
        ).unwrap();

        target_texture
    }
}

impl<'a> Drawable for Debayer<'a> {
    fn draw(&self, params: &mut DrawParams, pos: Pos) {
        let texture = Self::debayer(&self.raw_image, self.context);

        ShaderBox {
            fragment_shader: r#"
                #version 450
                uniform sampler2D in_image;
                in vec2 frag_position;
                out vec4 color;

                void main(void) {
                    ivec2 pos = ivec2(textureSize(in_image, 0) * ((frag_position.xy + vec2(1)) * vec2(.5)));
                    color = vec4(texelFetch(in_image, pos, 0));
                }
           "#.to_string(),
            uniforms: uniform! {
                in_image: &texture,
            },
        }.draw(params, pos);
    }
}
