use std::rc::Rc;

use glow::HasContext;

use crate::macros::macs::include_shader;

pub struct TriangleRenderer {
    gl: Rc<glow::Context>,
    program: glow::NativeProgram,
    vao: glow::NativeVertexArray,
    vbo: glow::NativeBuffer,
}

impl TriangleRenderer {
    pub unsafe fn new(gl: Rc<glow::Context>) -> Self {
        unsafe {
            let program = include_shader!(gl, "triangle");
            let vao = gl.create_vertex_array().expect("Cannot create VAO");
            let vbo = gl.create_buffer().expect("Cannot create VBO");

            Self {
                gl,
                program,
                vao,
                vbo,
            }
        }
    }

    pub unsafe fn resize(self) -> Self {
        unsafe {
            let gl = &self.gl;
            gl.delete_buffer(self.vbo);
            gl.delete_vertex_array(self.vao);

            let vao = gl.create_vertex_array().expect("Cannot create VAO");
            let vbo = gl.create_buffer().expect("Cannot create VBO");

            Self {
                gl: self.gl,
                program: self.program,
                vao,
                vbo,
            }
        }
    }

    pub unsafe fn render(
        &self,
        gl: &glow::Context,
        center: (f32, f32),
        size: f32,
        color: (f32, f32, f32),
    ) {
        // #[rustfmt::skip]
        // let vertices: [f32; 18] = [
        //     // x,    y,   z,   r,   g,   b
        //      0.0,  0.5, 0.0, 1.0, 0.0, 0.0,
        //     -0.5, -0.5, 0.0, 0.0, 1.0, 0.0,
        //      0.5, -0.5, 0.0, 0.0, 0.0, 1.0,
        // ];
        #[rustfmt::skip]
        let vertices: [f32; 18] = [
            // x,    y,   z,   r,   g,   b
            center.0             , center.1 + size / 2.0, 0.0, color.0, color.1, color.2,
            center.0 - size / 2.0, center.1 - size / 2.0, 0.0, color.0, color.1, color.2,
            center.0 + size / 2.0, center.1 - size / 2.0, 0.0, color.0, color.1, color.2,
        ];

        unsafe {
            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));

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

            gl.use_program(Some(self.program));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);

            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.bind_vertex_array(None);
        }
    }
}

impl Drop for TriangleRenderer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_buffer(self.vbo);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_program(self.program);
        }
    }
}

// Simple triangle data: position (x, y, z) + color (r, g, b)
// #[rustfmt::skip]
// let vertices: [f32; 18] = [
//     // x,    y,   z,   r,   g,   b
//      0.0,  0.5, 0.0, 1.0, 0.0, 0.0,
//     -0.5, -0.5, 0.0, 0.0, 1.0, 0.0,
//      0.5, -0.5, 0.0, 0.0, 0.0, 1.0,
// ];
//
// let font_bytes = include_font!("CaskaydiaCoveNerdFont-Regular.ttf");
// let font_size = 48.0;
//
// let mut text_renderer = unsafe { TextRenderer::new(&gl, font_bytes, font_size as u32) };
//
// let cache_width: u32 = 1024;
// let cache_height: u32 = 1024;
//
// let font_texture = unsafe { gl.create_texture().unwrap() };
//
// let text = "fi != -> ===";
//
// let triangle_program = unsafe { include_shader!(gl, "triangle") };
// let font_program = unsafe { include_shader!(gl, "font") };
// let vao = unsafe { gl.create_vertex_array().expect("Cannot create VAO") };
// let vbo = unsafe { gl.create_buffer().expect("Cannot create VBO") };
// let font_vao = unsafe { gl.create_vertex_array().expect("Cannot create VAO") };
// let font_vbo = unsafe { gl.create_buffer().expect("Cannot create VBO") };
//
// unsafe {
//     gl.bind_vertex_array(Some(vao));
//     gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
//
//     let vertex_bytes = std::slice::from_raw_parts(
//         vertices.as_ptr() as *const u8,
//         vertices.len() * std::mem::size_of::<f32>(),
//     );
//
//     gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertex_bytes, glow::STATIC_DRAW);
//
//     let stride = 6 * std::mem::size_of::<f32>() as i32;
//
//     // location = 0 -> vec3 position
//     gl.enable_vertex_attrib_array(0);
//     gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
//
//     // location = 1 -> vec3 color
//     gl.enable_vertex_attrib_array(1);
//     gl.vertex_attrib_pointer_f32(
//         1,
//         3,
//         glow::FLOAT,
//         false,
//         stride,
//         3 * std::mem::size_of::<f32>() as i32,
//     );
//
//     gl.bind_buffer(glow::ARRAY_BUFFER, None);
//     gl.bind_vertex_array(None);
//
//     gl.bind_buffer(glow::ARRAY_BUFFER, Some(font_vbo));
//     gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 0, 0);
//     gl.enable_vertex_attrib_array(0);
//
//     // uv buffer
//     gl.bind_vertex_array(Some(font_vao));
//     gl.bind_buffer(glow::ARRAY_BUFFER, Some(font_vbo));
//
//     gl.enable_vertex_attrib_array(0);
//     gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
//
//     gl.enable_vertex_attrib_array(1);
//     gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);
//
//     gl.bind_vertex_array(None);
// }
//
// let proj_loc = unsafe { gl.get_uniform_location(font_program, "u_proj") };
// let text_color_loc = unsafe { gl.get_uniform_location(font_program, "u_text_color") };
//
// let mut start_point: f32 = 0.0;
//
// while !window.should_close() {
//     let window_size = window.get_framebuffer_size();
//
//     for (_, event) in glfw::flush_messages(&events) {
//         match event {
//             WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
//                 window.set_should_close(true);
//             }
//             WindowEvent::FramebufferSize(width, height) => unsafe {
//                 gl.viewport(0, 0, width, height);
//             },
//             _ => {}
//         }
//     }
//
//     unsafe {
//         let proj = ortho(
//             window.get_framebuffer_size().0 as f32,
//             window.get_framebuffer_size().1 as f32,
//         );
//
//         gl.clear_color(0.0, 0.0, 0.0, 0.0);
//         gl.clear(glow::COLOR_BUFFER_BIT);
//
//         gl.use_program(Some(triangle_program));
//         gl.bind_vertex_array(Some(vao));
//         gl.draw_arrays(glow::TRIANGLES, 0, 3);
//
//         text_renderer.draw_text(
//             &gl,
//             "office != affine -> ligatures?",
//             start_point,
//             80.0,
//             font_size,
//             [1.0, 1.0, 1.0],
//             &proj,
//         );
//     }
//
//     window.swap_buffers();
//
//     start_point += 0.1;
//     glfw.wait_events();
// }
//
// unsafe {
//     gl.delete_buffer(vbo);
//     gl.delete_vertex_array(vao);
//     gl.delete_program(triangle_program);
// }
