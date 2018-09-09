extern crate glium;
use glium::Frame;
use glium::uniforms::Uniforms;
use glium::backend::Facade;
use glium::index;
use graphical::gl_util::{Vertex, CachingContext};

/// All drawable elements can be rendered with openGL
/// a GUI is a single Drawable, that can contain children
pub trait Drawable {
    fn draw(&self,  context: &mut CachingContext);
}

/// Vec<Drawable> is the main container. If you want to draw multiple things, use this.
impl<'a> Drawable for Vec<&'a Drawable> {
    fn draw(&self, context: &mut CachingContext) {
        for drawable in self {
            drawable.draw(context);
        }
    }
}

/// Draws a given fragment shader onto a given Box. Useful for building other Drawables
pub struct ShaderBox<U> where U : Uniforms {
    start: (f32, f32),
    size: (f32, f32),
    fragment_shader: String,
    uniforms: U,
}
impl<U> Drawable for ShaderBox<U> where U : Uniforms {
    fn draw(&self, context: &mut CachingContext) {
        let program = {
            let program = context.create_fragment_program(self.fragment_shader.as_str());
            match program {
                Ok(p) => p,
                _ => unimplemented!()
            }
        };

        let start = self.start;
        let size = self.size;
        let vertices = &Vertex::triangle_strip_surface(&*context.facade, (start.0, start.1, start.0 + size.0, start.1 + size.1));
        context.draw(
            vertices,
            &index::NoIndices(index::PrimitiveType::TriangleStrip),
            &program,
            &self.uniforms,
            &Default::default(),
        );
    }
}

/// Renders a simple colored Box. Useful for semi transparent overlays.
pub struct ColorBox {
    pub start: (f32, f32),
    pub size: (f32, f32),
    pub color: [f32; 4],
}
impl Drawable for ColorBox {
    fn draw(&self, context: &mut CachingContext) {
        ShaderBox {
            start: self.start,
            size: self.size,
            fragment_shader: r#"
                #version 450
                out vec4 color;
                in vec4 color

                void main(void) {
                    color = in_color;
                }
            "#.to_string(),
            uniforms: uniform!{
                in_color: self.color
            }
        }.draw(context);
    }
}

/// Draws a single glyph. Do not use this Directly
pub struct Letter {
    chr: char,
}
