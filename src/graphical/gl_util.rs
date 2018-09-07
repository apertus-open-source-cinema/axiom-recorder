extern crate glium;
use glium::*;
use glium::backend::Facade;

pub const PASSTHROUGH_VERTEX_SHADER_SRC : &str = r#"
    #version 140
    in vec2 position;
    void main() {
        gl_Position = vec4(position, 0.0, 1.0);
    }
"#;

pub const PASSTHROUGH_FRAGMENT_SHADER_SRC : &str = r#"
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
    pub fn create_triangle_strip_vertex_buffer(context: &Facade) -> VertexBuffer<Vertex> {
        let vertices: Vec<Vertex> = [[-1.0, 1.0], [1.0, 1.0], [-1.0, -1.0], [1.0, -1.0]]
            .iter().map(|elem| {Vertex {position: *elem}}).collect();

        VertexBuffer::new(context, &vertices).unwrap()
    }
}
