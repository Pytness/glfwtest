mod glyphs;
mod macros;
// mod temp;

use std::ffi::CString;

use glutin::{
    config::{Config, ConfigTemplateBuilder},
    context::{NotCurrentContext, PossiblyCurrentContext},
};

use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    raw_window_handle::HasWindowHandle,
    window::{Window, WindowId},
};

use glutin::{
    context::{ContextApi, ContextAttributesBuilder, Version},
    display::GetGlDisplay,
    prelude::*,
};

use glutin_winit::DisplayBuilder;
use winit::window::WindowAttributes;

struct App {
    template: ConfigTemplateBuilder,
    state: Option<AppState>,
    gl_display: GlDisplayCreationState,
    gl_context: Option<PossiblyCurrentContext>,
}

struct AppState {
    // gl_surface: Surface<WindowSurface>,
    window: Window,
}

struct Renderer {
    gl: glow::Context,
}

impl Renderer {
    fn new<D: GlDisplay>(gl_display: &D) -> Self {
        let gl = unsafe {
            glow::Context::from_loader_function(|s| {
                let symbol = CString::new(s).unwrap();
                gl_display.get_proc_address(symbol.as_c_str())
            })
        };

        Self { gl }
    }
}

impl App {
    fn new(template: ConfigTemplateBuilder, display_builder: DisplayBuilder) -> Self {
        Self {
            template,
            state: None,
            gl_display: GlDisplayCreationState::Builder(Box::new(display_builder)),
            gl_context: None,
        }
    }
}

enum GlDisplayCreationState {
    /// The display was not build yet.
    Builder(Box<DisplayBuilder>),
    /// The display was already created for the application.
    Init,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let (_window, _gl_config) = match &mut self.gl_display {
            GlDisplayCreationState::Builder(display_builder) => {
                let (window, gl_config) = match display_builder.clone().build(
                    event_loop,
                    self.template.clone(),
                    gl_config_picker,
                ) {
                    Ok((window, gl_config)) => (window.unwrap(), gl_config),
                    Err(e) => panic!("Failed to build display: {e}"),
                };

                self.gl_display = GlDisplayCreationState::Init;

                self.gl_context =
                    Some(create_gl_context(&window, &gl_config).treat_as_possibly_current());

                (window, gl_config)
            }
            GlDisplayCreationState::Init => {
                todo!()
            }
        };
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                // Draw.

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.
                // self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();

    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_transparency(true);

    let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes()));

    let mut app = App::new(template, display_builder);
    event_loop.run_app(&mut app);

    // Initialize GLFW
    // Load OpenGL function pointers through GLFW

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

    // unsafe {
    //     gl.delete_buffer(vbo);
    //     gl.delete_vertex_array(vao);
    //     gl.delete_program(triangle_program);
    // }
}

fn ortho(width: f32, height: f32) -> [f32; 16] {
    #[rustfmt::skip]
    return [
        2.0 / width, 0.0, 0.0, 0.0,
        0.0, -2.0 / height, 0.0, 0.0,
        0.0, 0.0, -1.0, 0.0,
        -1.0, 1.0, 0.0, 1.0,
    ];
}

fn window_attributes() -> WindowAttributes {
    Window::default_attributes()
        .with_transparent(true)
        .with_title("Glutin triangle gradient example (press Escape to exit)")
}

pub fn gl_config_picker(configs: Box<dyn Iterator<Item = Config> + '_>) -> Config {
    configs
        .reduce(|accum, config| {
            let transparency_check = config.supports_transparency().unwrap_or(false)
                & !accum.supports_transparency().unwrap_or(false);

            if transparency_check || config.num_samples() > accum.num_samples() {
                config
            } else {
                accum
            }
        })
        .unwrap()
}

fn create_gl_context(window: &Window, gl_config: &Config) -> NotCurrentContext {
    let raw_window_handle = window.window_handle().ok().map(|wh| wh.as_raw());

    // The context creation part.
    let context_attributes = ContextAttributesBuilder::new().build(raw_window_handle);

    // Since glutin by default tries to create OpenGL core context, which may not be
    // present we should try gles.
    let fallback_context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::Gles(None))
        .build(raw_window_handle);

    // There are also some old devices that support neither modern OpenGL nor GLES.
    // To support these we can try and create a 2.1 context.
    let legacy_context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(2, 1))))
        .build(raw_window_handle);

    // Reuse the uncurrented context from a suspended() call if it exists, otherwise
    // this is the first time resumed() is called, where the context still
    // has to be created.
    let gl_display = gl_config.display();

    unsafe {
        gl_display
            .create_context(gl_config, &context_attributes)
            .unwrap_or_else(|_| {
                gl_display
                    .create_context(gl_config, &fallback_context_attributes)
                    .unwrap_or_else(|_| {
                        gl_display
                            .create_context(gl_config, &legacy_context_attributes)
                            .expect("failed to create context")
                    })
            })
    }
}
