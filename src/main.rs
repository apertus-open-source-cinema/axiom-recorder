extern crate gtk;
extern crate epoxy;
extern crate gl;
extern crate shared_library;
extern crate gdk;

use gtk::prelude::*;

use self::gl::types::*;
use std::ffi::CStr;
use std::mem;
use std::process::exit;
use std::ptr;
use self::shared_library::dynamic_library::DynamicLibrary;
use gdk::GLContextExt;

mod preview;
mod video_source;

fn compile_shader(src: &str, ty: GLenum) -> Result<GLuint, String> {
    unsafe {
        let shader = gl::CreateShader(ty);
        // Attempt to compile the shader
        let psrc = src.as_ptr() as *const GLchar;
        let len = src.len() as GLint;
        gl::ShaderSource(shader, 1, &psrc, &len);
        gl::CompileShader(shader);
        // Get the compile status
        let mut status = epoxy::FALSE as GLint;
        gl::GetShaderiv(shader, epoxy::COMPILE_STATUS, &mut status);
        // Fail on error
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
        // Get the link status
        let mut status = epoxy::FALSE as GLint;
        gl::GetProgramiv(program, epoxy::LINK_STATUS, &mut status);
        // Fail on error
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


fn main() {
    if gtk::init().is_err() {
        println!("Failed to initialize GTK.");
        return;
    }

    let glade_src = include_str!("recorder.glade");
    let builder = gtk::Builder::new_from_string(glade_src);

    let window: gtk::Window = builder.get_object("main_window").unwrap();

    // show preferences dialog on button click
    let show_preferences: gtk::Button = builder.get_object("show_preferences").unwrap();
    let preferences_dialog: gtk::Dialog = builder.get_object("preferences_dialog").unwrap();
    show_preferences.connect_clicked(move |_| { preferences_dialog.show(); });

    // hide the preferences dialog on ok button click
    let close_preferences: gtk::Button = builder.get_object("close_preferences").unwrap();
    let preferences_dialog: gtk::Dialog = builder.get_object("preferences_dialog").unwrap();
    close_preferences.connect_clicked(move |_| { preferences_dialog.hide(); });

    // start the video Source
    let file_vs = video_source::FileVideoSource {
        path: "test.raw8".to_string(),
        width: 2304,
        height: 1296,
        bit_depth: 8,
    };

    let video_source = video_source::BufferedVideoSource::new(file_vs);

    // start the opengl rendering thread
    let glarea: gtk::GLArea = builder.get_object("gl_canvas").unwrap();
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


    /*
    let open_gl_handler = preview::OpenGlHandler::new(
        gl_canvas.get_context().unwrap(),
        video_source.subscribe()
    );

    open_gl_handler.start();
    */

    gtk::idle_add(clone!(glarea; || {
        glarea.queue_render();

        use std::{thread, time};

        let ten_millis = time::Duration::from_millis(10);
        let now = time::Instant::now();

        thread::sleep(ten_millis);

        Continue(true)
    }));

    epoxy::load_with(|s| unsafe {
        match DynamicLibrary::open(None).unwrap().symbol(s) {
            Ok(v) => v,
            Err(_) => ptr::null(),
        }
    });
    gl::load_with(epoxy::get_proc_addr);


    glarea.connect_realize(clone!(glarea; |_widget| {
            glarea.make_current();
             fn fatal_error(message: &str) {
                 // Can't gtk::main_quit as main loop isn't running, call exit
                println!("{}", message);
                exit(1);
            }
             let vertices: [GLfloat; 15] = [
                0.0, 0.5, 1.0, 0.0, 0.0,
                0.5, -0.5, 0.0, 1.0, 0.0,
                -0.5, -0.5, 0.0, 0.0, 1.0,
            ];
             let vert_shader_src = r#"
                #version 450
                 in vec2 position;
                 in vec3 color;
                 out vec3 vertex_color;
                 void main() {
                    vertex_color = color;
                    gl_Position = vec4(position + vec2(.5, .5), 0.0, 1.0);
                 }"#;
             let frag_shader_src = r#"
                #version 450
                 in vec3 vertex_color;
                 out vec4 color;
                 void main() {
                    color = vec4(vertex_color, 1.0);
                 }"#;
             let vs = match compile_shader(vert_shader_src, epoxy::VERTEX_SHADER) {
                Ok(v) => v,
                Err(e) => { fatal_error(&*format!("Error compiling vertex shader: {}", e)); 0 },
            };
            let fs = match compile_shader(frag_shader_src, epoxy::FRAGMENT_SHADER) {
                Ok(v) => v,
                Err(e) => { fatal_error(&*format!("Error compiling fragment shader: {}", e)); 0 },
            };
            let program = match link_program(vs, fs) {
                Ok(v) => v,
                Err(e) => { fatal_error(&*format!("Error linking shader: {}", e)); 0 },
            };

             let mut vao: GLuint = 0;
            let mut vbo: GLuint = 0;
             unsafe {
                gl::GenVertexArrays(1, &mut vao);
                gl::BindVertexArray(vao);
                 gl::GenBuffers(1, &mut vbo);
                gl::BindBuffer(epoxy::ARRAY_BUFFER, vbo);
                gl::BufferData(epoxy::ARRAY_BUFFER,
                              (vertices.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
                              mem::transmute(&vertices[0]),
                              epoxy::STATIC_DRAW);
                 gl::UseProgram(program);
                gl::BindFragDataLocation(program, 0, b"color\0".as_ptr() as *const GLchar);
            let pos_attr = gl::GetAttribLocation(program, b"position\0".as_ptr() as *const GLchar);
              gl::EnableVertexAttribArray(pos_attr as GLuint);
            gl::VertexAttribPointer(pos_attr as GLuint, 2, epoxy::FLOAT, epoxy::FALSE as GLboolean,
                                     (5 * mem::size_of::<GLfloat>()) as GLint,
                                       ptr::null());
            let color_attr = gl::GetAttribLocation(program, b"color\0".as_ptr() as *const GLchar);
              gl::EnableVertexAttribArray(color_attr as GLuint);
          gl::VertexAttribPointer(color_attr as GLuint, 3, epoxy::FLOAT, epoxy::FALSE as GLboolean,
                                  (5 * mem::size_of::<GLfloat>()) as GLint,
                                       (2 * mem::size_of::<GLfloat>()) as *const GLvoid);
            }
        }));


    glarea.connect_render(|_, _| {
        unsafe {
            gl::ClearColor(0.3, 0.3, 0.3, 1.0);
            gl::Clear(epoxy::COLOR_BUFFER_BIT);
            gl::DrawArrays(epoxy::TRIANGLES, 0, 3);
        };
        println!("render");

        Inhibit(false)
    });

    window.set_default_size(400, 400);
    window.show_all();

    gtk::main();
}
