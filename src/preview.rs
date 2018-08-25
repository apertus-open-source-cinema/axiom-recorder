extern crate libc;
extern crate gdk;
extern crate gl;
extern crate epoxy;
extern crate gtk;
extern crate shared_library;
extern crate bus;

use std::{mem, ptr};
use gtk::prelude::*;
use video_source::Image;
use self::gl::types::*;
use self::shared_library::dynamic_library::DynamicLibrary;
use std::process::exit;
use std::ffi::CStr;
use std::os::raw::c_void;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use self::bus::{BusReader};

pub struct OpenGlHandler {
    pub glarea: gtk::GLArea,
    pub source: Rc<RefCell<BusReader<Image>>>,
    
    vao: Rc<Cell<GLuint>>,
    vbo: Rc<Cell<GLuint>>,
    tex: Rc<Cell<GLuint>>,
}

macro_rules! clone {
($($n:ident),+; || $body:block) => (
    {
        $( let $n = $n.clone(); )+
        move || { $body }
    }
);
($($n:ident),+; |$($p:ident),+| $body:block) => (
    {
        $( let $n = $n.clone(); )+
        move |$($p),+| { $body }
    }
);
}

impl OpenGlHandler {
    pub fn new(glarea: gtk::GLArea, source: BusReader<Image>) -> OpenGlHandler {
        epoxy::load_with(|s| unsafe {
            match DynamicLibrary::open(None).unwrap().symbol(s) {
                Ok(v) => v,
                Err(_) => ptr::null(),
            }
        });
        gl::load_with(epoxy::get_proc_addr);

        
        let mut handler = OpenGlHandler {
            glarea: glarea,
            source: Rc::new(RefCell::new(source)),
            vao: Rc::new(Cell::new(0)),
            vbo: Rc::new(Cell::new(0)),
            tex: Rc::new(Cell::new(0)),
        };

        handler.init();

        handler
    }

    fn init(&mut self) {
        self.glarea.connect_realize({
            let glarea = self.glarea.clone();
            let vao = self.vao.clone();
            let vbo = self.vbo.clone();
            let tex = self.tex.clone();

            move |_widget| {
                glarea.make_current();

                fn fatal_error(message: &str) {
                    eprintln!("{}", message);
                    exit(1);
                }

                let vertices: [GLfloat; 12] = [
                    -1.0, -1.0,
                    -1.0,  1.0,
                     1.0,  1.0,
                    -1.0, -1.0,
                     1.0,  1.0,
                     1.0, -1.0
                ];

                let vert_shader_src = r#"
                    #version 450
                    in vec2 position;
                    void main() {
                        gl_Position = vec4(position, 1.0, 1.0);
                    }"#;

                let frag_shader_src = r#"
                    #version 450
                    uniform sampler2D raw_image;
                    out vec4 color;
                    
                    float get_intensity(ivec2 pos) {
                        return texelFetch(raw_image, pos, 0).r;
                    }
                    
                    vec3 get_color_value(ivec2 pos) {
                        int x = (pos.x/2)*2;
                        int y = (pos.y/2)*2;
                    
                        float r = get_intensity(ivec2(x + 1, y));
                        float g1 = get_intensity(ivec2(x, y));
                        float g2 = get_intensity(ivec2(x+1, y+1));
                        float b = get_intensity(ivec2(x, y + 1));
                    
                        vec3 col = vec3(r, (g1+g2)/2.0, b);
                        return col;
                    }
                    
                    
                    void main(void) {
                        ivec2 size = textureSize(raw_image, 0);
                        ivec2 icord = ivec2(gl_FragCoord);
                        ivec2 rotcord = ivec2(size.x - icord.x, icord.y);
                    
                        vec3 debayered = get_color_value(rotcord);
                        vec3 clamped = max(debayered, vec3(0.));
                        vec3 powed = pow(clamped, vec3(0.5 * 2.));
                        vec3 exposured = powed * 0.5 * 2.;
                    
                        // float i = get_intensity(ivec2(gl_FragCoord));
                        // color = vec4(i, i, i, 1.0);
                    
                        // pack the color into the gl_FragColor without transparency
                        color = vec4(exposured, 1.0);
                        // color = vec4(1.0, 0.0, 0.0, 1.0);
                    }"#;

                let vs = match OpenGlHandler::compile_shader(vert_shader_src, epoxy::VERTEX_SHADER) {
                    Ok(v) => v,
                    Err(e) => { fatal_error(&*format!("Error compiling vertex shader: {}", e)); 0 },
                };

                let fs = match OpenGlHandler::compile_shader(frag_shader_src, epoxy::FRAGMENT_SHADER) {
                    Ok(v) => v,
                    Err(e) => { fatal_error(&*format!("Error compiling fragment shader: {}", e)); 0 },
                };

                let program = match OpenGlHandler::link_program(vs, fs) {
                    Ok(v) => v,
                    Err(e) => { fatal_error(&*format!("Error linking shader: {}", e)); 0 },
                };


                unsafe {
                    let mut tmp = 0;
                    gl::GenVertexArrays(1, &mut tmp);
                    vao.set(tmp);
                    // gl::GenVertexArrays(1, &mut self.vao);

                    gl::BindVertexArray(vao.get());
                    gl::GenBuffers(1, &mut tmp);
                    vbo.set(tmp);
                    gl::BindBuffer(epoxy::ARRAY_BUFFER, vbo.get());
                    gl::BufferData(epoxy::ARRAY_BUFFER,
                                   (vertices.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
                                   mem::transmute(&vertices[0]),
                                   epoxy::STATIC_DRAW);

                    gl::UseProgram(program);
                    gl::BindFragDataLocation(program, 0, b"color\0".as_ptr() as *const GLchar);

                    let pos_attr = gl::GetAttribLocation(program, b"position\0".as_ptr() as *const GLchar);
                    gl::EnableVertexAttribArray(pos_attr as GLuint);
                    gl::VertexAttribPointer(pos_attr as GLuint, 2, epoxy::FLOAT, epoxy::FALSE as GLboolean, 
                                            0, ptr::null());


                    gl::CreateTextures(epoxy::TEXTURE_2D, 1, &mut tmp);
                    tex.set(tmp);
                    gl::BindTexture(epoxy::TEXTURE_2D, tex.get());
                    gl::TexParameteri(epoxy::TEXTURE_2D, epoxy::TEXTURE_MAG_FILTER, epoxy::NEAREST as i32);
                    gl::TexParameteri(epoxy::TEXTURE_2D, epoxy::TEXTURE_MIN_FILTER, epoxy::NEAREST as i32);
                    gl::TexParameteri(epoxy::TEXTURE_2D, epoxy::TEXTURE_WRAP_S, epoxy::CLAMP_TO_EDGE as i32);
                    gl::TexParameteri(epoxy::TEXTURE_2D, epoxy::TEXTURE_WRAP_T, epoxy::CLAMP_TO_EDGE as i32);
                }
            }
        });


        self.glarea.connect_render(|_, _| {
            unsafe {
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(epoxy::COLOR_BUFFER_BIT);

                gl::DrawArrays(epoxy::TRIANGLES, 0, 6);
            };

            Inhibit(false)
        });


        gtk::idle_add({
            let glarea = self.glarea.clone();
            let source = self.source.clone();
            let tex = self.tex.clone();
            
            move || {
                match source.borrow_mut().try_recv() {
                    Ok(img) => {
                        unsafe {
                            gl::Viewport(0, 0, img.width as i32, img.height as i32);

                            gl::BindTexture(epoxy::TEXTURE_2D, tex.get());
                            gl::TexImage2D(epoxy::TEXTURE_2D, 0, epoxy::R8 as i32,
                                           img.width as i32, img.height as i32,
                                           0, epoxy::RED, epoxy::UNSIGNED_BYTE,
                                           img.data.as_ptr() as *const c_void);
                        }
                    }
                    Err(e) => {}
                }

                glarea.queue_render();

                use std::{thread, time};
                thread::sleep(time::Duration::from_millis(1));

                Continue(true)
            }
        });
    }

    pub fn updateFragmentShader(&self) {}

    fn compile_shader(src: &str, ty: GLenum) -> Result<GLuint, String> {
        unsafe {
            let shader = gl::CreateShader(ty);
            let psrc = src.as_ptr() as *const GLchar;
            let len = src.len() as GLint;
    
            gl::ShaderSource(shader, 1, &psrc, &len);
            gl::CompileShader(shader);
    
            let mut status = epoxy::FALSE as GLint;
            gl::GetShaderiv(shader, epoxy::COMPILE_STATUS, &mut status);
    
            if status != (epoxy::TRUE as GLint) {
                let mut len = 0;
                gl::GetShaderiv(shader, epoxy::INFO_LOG_LENGTH, &mut len);
    
                let mut buf = vec![0i8; len as usize];
                gl::GetShaderInfoLog(
                    shader,
                    len,
                    ptr::null_mut(),
                    buf.as_mut_ptr() as *mut GLchar,
                );
    
                return Err(CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned());
            }
    
            Ok(shader)
        }
    }
    
    fn link_program(vs: GLuint, fs: GLuint) -> Result<GLuint, String> {
        unsafe {
            let program = gl::CreateProgram();
    
            gl::AttachShader(program, vs);
            gl::AttachShader(program, fs);
    
            gl::LinkProgram(program);
    
            let mut status = epoxy::FALSE as GLint;
            gl::GetProgramiv(program, epoxy::LINK_STATUS, &mut status);
    
            if status != (epoxy::TRUE as GLint) {
                let mut len: GLint = 0;
                gl::GetProgramiv(program, epoxy::INFO_LOG_LENGTH, &mut len);
    
                let mut buf = vec![0i8; len as usize];
                gl::GetProgramInfoLog(
                    program,
                    len,
                    ptr::null_mut(),
                    buf.as_mut_ptr() as *mut GLchar,
                );
    
                return Err(CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned());
            }
    
            Ok(program)
        }
    }
}
