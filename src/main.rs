use glfw::{Action, Context as _, Key, WindowEvent, WindowHint, WindowMode};
use glow::HasContext;

macro_rules! assets_path {
    ($name: literal) => {
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/", $name)
    };
}

macro_rules! include_shader {
    ($gl: ident, $name: literal ) => {{
        let gl = &$gl;
        let program = gl.create_program().expect("Cannot create program");

        let vs_source = include_str!(concat!(
            assets_path!("shaders/"),
            $name,
            "/",
            $name,
            ".vert"
        ));

        let fs_source = include_str!(concat!(
            assets_path!("shaders/"),
            $name,
            "/",
            $name,
            ".frag"
        ));

        let vs = gl
            .create_shader(glow::VERTEX_SHADER)
            .expect("Cannot create vertex shader");

        gl.shader_source(vs, vs_source);
        gl.compile_shader(vs);

        if !gl.get_shader_compile_status(vs) {
            panic!(
                "Vertex shader compilation failed:\n{}",
                gl.get_shader_info_log(vs)
            );
        }

        let fs = gl
            .create_shader(glow::FRAGMENT_SHADER)
            .expect("Cannot create fragment shader");
        gl.shader_source(fs, fs_source);
        gl.compile_shader(fs);
        if !gl.get_shader_compile_status(fs) {
            panic!(
                "Fragment shader compilation failed:\n{}",
                gl.get_shader_info_log(fs)
            );
        }

        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            panic!("Program link failed:\n{}", gl.get_program_info_log(program));
        }

        gl.delete_shader(vs);
        gl.delete_shader(fs);

        program
    }};
}

fn main() {
    // Initialize GLFW
    let mut glfw = glfw::init(glfw::fail_on_errors).expect("Failed to init GLFW");

    // Request an OpenGL 3.3 Core context
    glfw.window_hint(WindowHint::ContextVersion(3, 3));
    glfw.window_hint(WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));

    let (mut window, events) = glfw
        .create_window(800, 600, "GLFW + glow", WindowMode::Windowed)
        .expect("Failed to create window");

    window.make_current();
    window.set_key_polling(true);
    window.set_framebuffer_size_polling(true);

    // Load OpenGL function pointers through GLFW
    let gl = unsafe {
        glow::Context::from_loader_function(|s| {
            window
                .get_proc_address(s)
                .map_or(std::ptr::null(), |p| p as *const _)
        })
    };

    // Simple triangle data: position (x, y, z) + color (r, g, b)
    let vertices: [f32; 18] = [
        // x,    y,    z,    r,    g,    b
        0.0, 0.5, 0.0, 1.0, 0.0, 0.0, -0.5, -0.5, 0.0, 0.0, 1.0, 0.0, 0.5, -0.5, 0.0, 0.0, 0.0, 1.0,
    ];

    let program = unsafe { include_shader!(gl, "triangle") };
    let vao = unsafe { gl.create_vertex_array().expect("Cannot create VAO") };
    let vbo = unsafe { gl.create_buffer().expect("Cannot create VBO") };

    unsafe {
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));

        let vertex_bytes = std::slice::from_raw_parts(
            vertices.as_ptr() as *const u8,
            vertices.len() * std::mem::size_of::<f32>(),
        );

        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertex_bytes, glow::STATIC_DRAW);

        let stride = 6 * std::mem::size_of::<f32>() as i32;

        // location = 0 -> vec3 position
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);

        // location = 1 -> vec3 color
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(
            1,
            3,
            glow::FLOAT,
            false,
            stride,
            3 * std::mem::size_of::<f32>() as i32,
        );

        gl.bind_buffer(glow::ARRAY_BUFFER, None);
        gl.bind_vertex_array(None);
    }

    while !window.should_close() {
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    window.set_should_close(true);
                }
                WindowEvent::FramebufferSize(width, height) => unsafe {
                    gl.viewport(0, 0, width, height);
                },
                _ => {}
            }
        }

        unsafe {
            gl.clear_color(0.1, 0.12, 0.15, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);

            gl.use_program(Some(program));
            gl.bind_vertex_array(Some(vao));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
        }

        window.swap_buffers();
    }

    unsafe {
        gl.delete_buffer(vbo);
        gl.delete_vertex_array(vao);
        gl.delete_program(program);
    }
}
