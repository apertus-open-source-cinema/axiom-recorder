extern crate glium;

use glium::backend::Facade;
use glium::index::IndicesSource;
use glium::uniforms::Uniforms;
use glium::vertex::MultiVerticesSource;
use glium::Frame;
use glium::Program;
use glium::*;
use std::collections::BTreeMap;
use std::ops::Deref;

pub const PASSTHROUGH_VERTEX_SHADER_SRC: &str = r#"
    #version 140
    in vec2 relative_position;
    in vec2 absolute_position;
    out vec2 position;

    void main() {
        position = relative_position;
        gl_Position = vec4(absolute_position, 0.0, 1.0);
    }
"#;

#[derive(Copy, Clone)]
pub struct Vertex {
    absolute_position: [f32; 2],
    relative_position: [f32; 2],
}
implement_vertex!(Vertex, absolute_position, relative_position);

impl Vertex {
    /// Creates the Vertices, which will result in a Rectangle, if drawn as triangle strip
    /// The coordinates are normalized to a range from 0 to 1
    pub fn triangle_strip_surface(
        context: &Facade,
        corners: (f32, f32, f32, f32),
    ) -> VertexBuffer<Vertex> {
        let start = (corners.0 * 2. - 1., corners.1 * 2. - 1.);
        let end = (corners.2 * 2. - 1., corners.3 * 2. - 1.);

        let vertices: Vec<Vertex> = vec![
            ([start.0, start.1], [0., 0.]),
            ([end.0, start.1], [1., 0.]),
            ([start.0, end.1], [0., 1.]),
            ([end.0, end.1], [1., 1.]),
        ].iter()
        .map(|(absolute, relative)| Vertex {
            absolute_position: *absolute,
            relative_position: *relative,
        }).collect();

        VertexBuffer::new(context, &vertices).unwrap()
    }
}
