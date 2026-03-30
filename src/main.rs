use glfw::{Action, Context as _, Key, WindowEvent, WindowHint, WindowMode};
use glow::{HasContext, NativeTexture};
use rusttype::gpu_cache::Cache;
use rusttype::{Font, PositionedGlyph, Scale, point};
use std::thread::sleep;
use std::time::Duration;

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

macro_rules! include_font {
    ($name: literal) => {{
        let font_data = include_bytes!(concat!(assets_path!("fonts/"), $name));
        Font::try_from_bytes(font_data as &[u8]).expect("Failed to load font")
    }};
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

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

    let font = include_font!("CaskaydiaCoveNerdFont-Regular.ttf");
    let scale = Scale::uniform(50.0);
    let v_metrics = font.v_metrics(scale);

    let cache_width = 1024u32;
    let cache_height = 1024u32;

    let font_texture = unsafe { gl.create_texture().unwrap() };

    let mut cache = build_font_cache(&gl, font_texture, cache_width, cache_height);

    let text = "Hello world";
    let start = point(20.0, 50.0 + v_metrics.ascent);

    let glyphs: Vec<_> = font.layout(text, scale, start).collect();

    cache_glyphs(&gl, &mut cache, font_texture, &glyphs);

    let triangle_program = unsafe { include_shader!(gl, "triangle") };
    let font_program = unsafe { include_shader!(gl, "font") };
    let vao = unsafe { gl.create_vertex_array().expect("Cannot create VAO") };
    let vbo = unsafe { gl.create_buffer().expect("Cannot create VBO") };
    let font_vao = unsafe { gl.create_vertex_array().expect("Cannot create VAO") };
    let font_vbo = unsafe { gl.create_buffer().expect("Cannot create VBO") };

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

        gl.bind_buffer(glow::ARRAY_BUFFER, Some(font_vbo));
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 0, 0);
        gl.enable_vertex_attrib_array(0);

        // uv buffer
        gl.bind_vertex_array(Some(font_vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(font_vbo));

        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);

        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);

        gl.bind_vertex_array(None);
    }

    let proj_loc = unsafe { gl.get_uniform_location(font_program, "u_proj") };
    let text_color_loc = unsafe { gl.get_uniform_location(font_program, "u_text_color") };

    let mut start_point: f32 = 0.0;

    while !window.should_close() {
        let window_size = window.get_framebuffer_size();

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
            let proj = ortho(
                window.get_framebuffer_size().0 as f32,
                window.get_framebuffer_size().1 as f32,
            );
            gl.clear_color(0.1, 0.12, 0.15, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);

            gl.use_program(Some(triangle_program));
            gl.bind_vertex_array(Some(vao));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);

            let start = point(
                start_point,
                (window_size.1 as f32 / 2.0).ceil() + v_metrics.ascent,
            );

            let glyphs: Vec<_> = font.layout(text, scale, start).collect();
            let font_vertices: Vec<Vertex> = glyphs
                .iter()
                .filter_map(|glyph| get_glyph_rect(&cache, glyph))
                .flatten()
                .collect();

            gl.use_program(Some(font_program));
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(font_vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&font_vertices),
                glow::DYNAMIC_DRAW,
            );
            gl.uniform_matrix_4_f32_slice(proj_loc.as_ref(), false, &proj);
            gl.uniform_3_f32(text_color_loc.as_ref(), 1.0, 0.0, 1.0);

            gl.bind_vertex_array(Some(font_vao));

            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(font_texture));

            gl.draw_arrays(glow::TRIANGLES, 0, font_vertices.len() as i32);
        }

        window.swap_buffers();
        start_point += 1.0;
        sleep(Duration::from_millis(16));
    }

    unsafe {
        gl.delete_buffer(vbo);
        gl.delete_vertex_array(vao);
        gl.delete_program(triangle_program);
    }
}

fn ortho(width: f32, height: f32) -> [f32; 16] {
    [
        2.0 / width,
        0.0,
        0.0,
        0.0,
        0.0,
        -2.0 / height,
        0.0,
        0.0,
        0.0,
        0.0,
        -1.0,
        0.0,
        -1.0,
        1.0,
        0.0,
        1.0,
    ]
}

fn build_font_cache(
    gl: &glow::Context,
    texture: NativeTexture,
    cache_width: u32,
    cache_height: u32,
) -> Cache {
    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::R8 as i32,
            cache_width as i32,
            cache_height as i32,
            0,
            glow::RED,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(&vec![0u8; (cache_width * cache_height) as usize])),
        );

        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
    }

    let mut cache = Cache::builder()
        .dimensions(cache_width, cache_height)
        .build();

    cache
}

fn cache_glyphs<'a>(
    gl: &glow::Context,
    cache: &mut Cache<'a>,
    texture: NativeTexture,
    glyphs: &[PositionedGlyph<'a>],
) {
    for glyph in glyphs {
        cache.queue_glyph(0, glyph.clone());
    }

    cache
        .cache_queued(|rect, data| unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                rect.min.x as i32,
                rect.min.y as i32,
                rect.width() as i32,
                rect.height() as i32,
                glow::RED,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(data)),
            );
        })
        .unwrap();
}

fn get_glyph_rect<'a>(cache: &Cache<'a>, glyph: &PositionedGlyph<'a>) -> Option<[Vertex; 6]> {
    if let Ok(Some((uv_rect, screen_rect))) = cache.rect_for(0, glyph) {
        let x0 = screen_rect.min.x as f32;
        let y0 = screen_rect.min.y as f32;
        let x1 = screen_rect.max.x as f32;
        let y1 = screen_rect.max.y as f32;

        let u0 = uv_rect.min.x;
        let v0 = uv_rect.min.y;
        let u1 = uv_rect.max.x;
        let v1 = uv_rect.max.y;

        Some([
            Vertex {
                pos: [x0, y0],
                uv: [u0, v0],
            },
            Vertex {
                pos: [x1, y0],
                uv: [u1, v0],
            },
            Vertex {
                pos: [x1, y1],
                uv: [u1, v1],
            },
            Vertex {
                pos: [x0, y0],
                uv: [u0, v0],
            },
            Vertex {
                pos: [x1, y1],
                uv: [u1, v1],
            },
            Vertex {
                pos: [x0, y1],
                uv: [u0, v1],
            },
        ])
    } else {
        None
    }
}
