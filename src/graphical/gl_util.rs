extern crate glium;

use glium::*;
use glium::backend::Facade;
use glium::Program;
use std::ops::Deref;
use std::collections::BTreeMap;
use glium::uniforms::Uniforms;
use glium::index::IndicesSource;
use glium::vertex::MultiVerticesSource;
use glium::Frame;

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
    pub fn triangle_strip_surface(context: &Facade, corners: (f32, f32, f32, f32)) -> VertexBuffer<Vertex> {
        let vertices: Vec<Vertex> = [[corners.0, corners.1], [corners.0, corners.3], [corners.2, corners.3], [corners.2, corners.1]]
            .iter().map(|elem| { Vertex { position: *elem } }).collect();

        VertexBuffer::new(context, &vertices).unwrap()
    }
}

pub struct CachingContext {
    pub facade: Box<Facade>,
    pub surface: Frame,

    program_cache: BTreeMap<String, Result<Program, ProgramCreationError>>,
}

impl CachingContext {
    pub fn create_program(&mut self, vertex_shader: &str, fragment_shader: &str, geometry_shader: Option<&str>) -> &Result<Program, ProgramCreationError> {
        let cache_key = &format!("{}{}{:?}", vertex_shader, fragment_shader, geometry_shader);
        if !self.program_cache.contains_key(cache_key) {
            self.program_cache.insert(cache_key.to_string(), Program::from_source(
                &*self.facade,
                vertex_shader,
                fragment_shader,
                geometry_shader,
            ));
        }
        self.program_cache.get(cache_key).unwrap()
    }

    pub fn create_fragment_program(&mut self, fragment_shader: &str) -> &Result<Program, ProgramCreationError> {
        self.create_program(
            PASSTHROUGH_FRAGMENT_SHADER_SRC,
            fragment_shader,
            None,
        )
    }

    pub fn draw<'a, 'b, V, I, U>(
        &mut self, vs: V,
        is: I,
        program: &Program,
        uniforms: &U,
        draw_parameters: &DrawParameters,
    ) where V: MultiVerticesSource<'b>,
            I: Into<IndicesSource<'a>>,
            U: Uniforms {
        self.surface.draw(vs, is, program, uniforms, draw_parameters).unwrap();
    }
}
