use std::{ffi::CString, num::NonZeroU32, rc::Rc};

use glow::HasContext;
use glutin::{
    config::ConfigTemplateBuilder,
    surface::{Surface, WindowSurface},
};

use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use glutin::{display::GetGlDisplay, prelude::*};

use glutin_winit::{DisplayBuilder, GlWindow};

use crate::{
    font_registry::FontRegistry,
    renderers::{self, TextRenderer},
    text_manager::TextManager,
};

use crate::{gl_handler::GlHandler, macros::macs::include_font};

use std::time::Instant;

const FONT_SIZE: u32 = 20;
// const TEXT: &str = "abcdefghijklmnopqrstuvwxyz0123456789";
// const TEXT: &str = "a ---- <- -> <= << <= ------------ b";
const TEXT: &str = "a -<- <= b🤔";

pub struct App<'a> {
    gl_handler: GlHandler,
    state: Option<AppState>,
    gl: Option<Rc<glow::Context>>,
    font_registry: FontRegistry,
    text_renderer: Option<TextRenderer<'a>>,
    triangle_renderer: Option<renderers::TriangleRenderer>,
    quad_renderer: Option<renderers::QuadRenderer>,
    text_manager: Option<TextManager>,
    text_index: usize,
}

struct AppState {
    gl_surface: Surface<WindowSurface>,
    window: Window,
}

impl<'a> App<'a> {
    pub fn new(template: ConfigTemplateBuilder, display_builder: DisplayBuilder) -> Self {
        let mut font_registry = FontRegistry::new();

        font_registry.register_font(
            "CaskaydiaCoveNerdFont-Regular.ttf",
            include_font!("CaskaydiaCoveNerdFont-Regular.ttf"),
        );

        Self {
            gl_handler: GlHandler::new(template, display_builder),
            state: None,
            gl: None,
            font_registry,
            text_renderer: None,
            triangle_renderer: None,
            quad_renderer: None,
            text_manager: None,
            text_index: 0,
        }
    }

    /// SAFETY: This function should only be called after the OpenGL context has been created and made current in the `resumed` method.
    /// Calling this function before that will result in undefined behavior.
    pub unsafe fn gl(&self) -> &glow::Context {
        self.gl.as_ref().unwrap()
    }
}

impl<'a> ApplicationHandler for App<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let gl_window = self.gl_handler.get_or_create_gl_window(event_loop);

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

        let gl_context = self.gl_handler.gl_context.as_ref().unwrap();
        gl_context.make_current(&gl_surface).unwrap();

        let gl = unsafe {
            glow::Context::from_loader_function(|s| {
                let symbol = CString::new(s).unwrap();
                gl_config.display().get_proc_address(symbol.as_c_str())
            })
        };

        self.gl = Some(Rc::new(gl));

        self.triangle_renderer.get_or_insert_with(|| unsafe {
            renderers::TriangleRenderer::new(self.gl.as_ref().unwrap().clone())
        });
        // self.text_renderer.get_or_insert_with(|| unsafe {
        //     TextRenderer::new(
        //         self.gl.as_ref().unwrap().clone(),
        //         &self.font_registry,
        //         FONT_SIZE,
        //     )
        // });

        // FontRegistry must outlive TextRenderer
        self.text_renderer.get_or_insert_with(|| unsafe {
            // This is safe because the font registry is owned by the App struct
            // and will not be dropped while the TextRenderer is still in use.
            let font_registry: &'a FontRegistry = &*(&self.font_registry as *const _);

            TextRenderer::<'a>::new(self.gl.as_ref().unwrap().clone(), &font_registry, FONT_SIZE)
        });

        self.quad_renderer.get_or_insert_with(|| unsafe {
            renderers::QuadRenderer::new(
                self.gl.as_ref().unwrap().clone(),
                window.inner_size().width as i32,
                window.inner_size().height as i32,
            )
        });

        let font_size = self.text_renderer.as_ref().unwrap().font_size();
        self.text_manager.get_or_insert_with(|| {
            TextManager::new(
                font_size.0 as i32,
                font_size.1 as i32,
                window.inner_size().width as i32,
                window.inner_size().height as i32,
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
                let start = Instant::now();
                // Some platforms like EGL require resizing GL surface to update the size
                // Notable platforms here are Wayland and macOS, other don't require it
                // and the function is no-op, but it's wise to resize it for portability
                // reasons.
                if let Some(AppState { gl_surface, window }) = self.state.as_ref() {
                    let gl_context = self.gl_handler.gl_context.as_ref().unwrap();
                    gl_surface.resize(
                        gl_context,
                        NonZeroU32::new(size.width).unwrap(),
                        NonZeroU32::new(size.height).unwrap(),
                    );

                    unsafe {
                        let font_size = self.text_renderer.as_ref().unwrap().font_size();

                        self.text_manager = Some(TextManager::new(
                            font_size.0 as i32,
                            font_size.1 as i32,
                            size.width as i32,
                            size.height as i32,
                        ));

                        self.quad_renderer = Some(renderers::QuadRenderer::new(
                            self.gl.as_ref().unwrap().clone(),
                            size.width as i32,
                            size.height as i32,
                        ));

                        self.gl()
                            .viewport(0, 0, size.width as i32, size.height as i32);

                        let proj = ortho(size.width as f32, size.height as f32);

                        let text_manager = self.text_manager.as_ref().unwrap();

                        let chars = &TEXT.chars().collect::<Vec<_>>();

                        let mut string_buffer = String::with_capacity(TEXT.len());

                        self.quad_renderer.as_ref().unwrap().with(|| {
                            let mut index = 0;

                            for row in 0..text_manager.rows {
                                let mut col = 0;

                                while col <= text_manager.cols as usize {
                                    let text_size =
                                        (chars.len() - index).min(text_manager.cols as usize - col);
                                    if text_size == 0 {
                                        break;
                                    }

                                    string_buffer.clear();
                                    string_buffer.extend(&chars[index..index + text_size]);

                                    let cell_position =
                                        text_manager.get_cell_position(row, col as i32);

                                    self.text_renderer.as_mut().unwrap().draw_text(
                                        &string_buffer,
                                        cell_position.x as f32,
                                        cell_position.y as f32,
                                        [1.0, 1.0, 1.0],
                                        &proj,
                                    );

                                    col += text_size;
                                    index += text_size;

                                    if index >= chars.len() {
                                        index %= chars.len();
                                    }
                                }
                            }
                        });
                    }

                    window.request_redraw();
                }

                let duration = start.elapsed();
                println!("Resized in {} ms", duration.as_millis());
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let start = Instant::now();
                if let Some(AppState { gl_surface, window }) = &self.state {
                    let gl_context = self.gl_handler.gl_context.as_ref().unwrap();

                    unsafe {
                        // let height = window.inner_size().height as i32;
                        //
                        // let points = [(0, 0), (0, 2), (1, 1), (1, 3)];
                        // for point in points {
                        //     let cell_box = self
                        //         .text_manager
                        //         .as_ref()
                        //         .unwrap()
                        //         .get_cell_box(point.0, point.1);
                        //
                        //     self.quad_renderer.as_ref().unwrap().clear_section(
                        //         cell_box.x,
                        //         height - cell_box.y,
                        //         cell_box.width,
                        //         cell_box.height,
                        //     );
                        // }

                        self.quad_renderer.as_ref().unwrap().render();
                    }

                    gl_surface.swap_buffers(gl_context).unwrap();
                }

                let duration = start.elapsed();
                println!("Redrawn in {} ms", duration.as_millis());
            }

            WindowEvent::MouseInput {
                device_id: _device_id,
                state,
                button,
            } => {
                if state == winit::event::ElementState::Pressed
                    && button == winit::event::MouseButton::Left
                {
                    let window = &self.state.as_ref().unwrap().window;

                    if self.text_index < TEXT.len() {
                        let text_manager = self.text_manager.as_ref().unwrap();

                        let cell_position =
                            text_manager.get_cell_position(0, self.text_index as i32);

                        let proj = ortho(
                            window.inner_size().width as f32,
                            window.inner_size().height as f32,
                        );

                        unsafe {
                            self.quad_renderer.as_ref().unwrap().with(|| {
                                self.text_renderer.as_mut().unwrap().draw_text(
                                    &TEXT[self.text_index..self.text_index + 1],
                                    cell_position.x as f32,
                                    cell_position.y as f32,
                                    [1.0, 1.0, 1.0],
                                    &proj,
                                );
                            })
                        }

                        self.text_index += 1;
                    }

                    // window.request_redraw();
                }
            }
            _ => (),
        }
    }
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
