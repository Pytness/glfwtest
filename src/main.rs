mod glyphs;
mod macros;

use std::{ffi::CString, num::NonZeroU32};

use glow::HasContext;
use glutin::{
    config::{Config, ConfigTemplateBuilder, GetGlConfig},
    context::{NotCurrentContext, PossiblyCurrentContext},
    surface::{Surface, WindowSurface},
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

use glutin_winit::{DisplayBuilder, GlWindow};
use winit::window::WindowAttributes;

use self::macros::macs::include_font;

struct App {
    template: ConfigTemplateBuilder,
    state: Option<AppState>,
    gl_display: GlDisplayCreationState,
    gl_context: Option<PossiblyCurrentContext>,
    gl: Option<glow::Context>,
    text_renderer: Option<glyphs::TextRenderer>,
}

struct AppState {
    gl_surface: Surface<WindowSurface>,
    window: Window,
}

impl App {
    fn new(template: ConfigTemplateBuilder, display_builder: DisplayBuilder) -> Self {
        Self {
            template,
            state: None,
            gl_display: GlDisplayCreationState::Builder(Box::new(display_builder)),
            gl_context: None,
            gl: None,
            text_renderer: None,
        }
    }

    fn get_or_create_gl_window(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Option<(Window, Config)> {
        match &mut self.gl_display {
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

                Some((window, gl_config))
            }
            GlDisplayCreationState::Init => {
                let gl_config = self.gl_context.as_ref().unwrap().config();

                match glutin_winit::finalize_window(event_loop, window_attributes(), &gl_config) {
                    Ok(window) => Some((window, gl_config)),
                    Err(_) => {
                        // self.exit_state = Err(err.into());
                        event_loop.exit();
                        None
                    }
                }
            }
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
        let gl_window = self.get_or_create_gl_window(event_loop);

        if gl_window.is_none() {
            return;
        }

        let (window, gl_config) = gl_window.unwrap();

        let attrs = window
            .build_surface_attributes(Default::default())
            .expect("Failed to build surface attributes");

        let gl_surface = unsafe {
            gl_config
                .display()
                .create_window_surface(&gl_config, &attrs)
                .expect("Failed to create surface")
        };

        let gl_context = self.gl_context.as_ref().unwrap();
        gl_context.make_current(&gl_surface).unwrap();

        let gl = unsafe {
            glow::Context::from_loader_function(|s| {
                let symbol = CString::new(s).unwrap();
                gl_config.display().get_proc_address(symbol.as_c_str())
            })
        };

        self.gl = Some(gl);

        self.text_renderer.get_or_insert_with(|| unsafe {
            glyphs::TextRenderer::new(
                self.gl.as_ref().unwrap(),
                include_font!("CaskaydiaCoveNerdFont-Regular.ttf"),
                48,
            )
        });

        self.state = Some(AppState { gl_surface, window });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) if size.width != 0 && size.height != 0 => {
                // Some platforms like EGL require resizing GL surface to update the size
                // Notable platforms here are Wayland and macOS, other don't require it
                // and the function is no-op, but it's wise to resize it for portability
                // reasons.
                if let Some(AppState {
                    gl_surface,
                    window: _,
                }) = self.state.as_ref()
                {
                    let gl_context = self.gl_context.as_ref().unwrap();
                    gl_surface.resize(
                        gl_context,
                        NonZeroU32::new(size.width).unwrap(),
                        NonZeroU32::new(size.height).unwrap(),
                    );

                    unsafe {
                        self.gl.as_ref().unwrap().viewport(
                            0,
                            0,
                            size.width as i32,
                            size.height as i32,
                        );
                    }
                }
            }
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

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(AppState { gl_surface, window }) = &self.state {
            let proj = ortho(
                window.inner_size().width as f32,
                window.inner_size().height as f32,
            );

            let gl_context = self.gl_context.as_ref().unwrap();
            unsafe {
                self.text_renderer.as_ref().unwrap().draw_text(
                    self.gl.as_ref().unwrap(),
                    "office != affine -> ligatures?",
                    80.0,
                    80.0,
                    48.0,
                    [1.0, 1.0, 1.0],
                    &proj,
                );
            }

            window.request_redraw();
            gl_surface.swap_buffers(gl_context).unwrap();
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
