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
    in vec2 position;
    out vec2 surface_position;

    void main() {
        surface_position = position;
        gl_Position = vec4(position, 0.0, 1.0);
    }
"#;

pub const PASSTHROUGH_FRAGMENT_SHADER_SRC: &str = r#"
    #version 140
    out vec4 color;
    void main() {
        color = vec4(1.0, 0.0, 0.0, 1.0);
    }
"#;

#[derive(Copy, Clone)]
pub struct Vertex {
    position: [f32; 2],
}
implement_vertex!(Vertex, position);

impl Vertex {
    pub fn triangle_strip_surface(
        context: &Facade,
        corners: (f32, f32, f32, f32),
    ) -> VertexBuffer<Vertex> {
        let vertices: Vec<Vertex> = [
            [corners.0, corners.1],
            [corners.2, corners.1],
            [corners.0, corners.3],
            [corners.2, corners.3],
        ]
            .iter()
            .map(|elem| Vertex { position: *elem })
            .collect();

        VertexBuffer::new(context, &vertices).unwrap()
    }
}
