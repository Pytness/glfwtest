use std::{ffi::CString, num::NonZeroU32, rc::Rc};

use glow::HasContext;
use glutin::{
    config::ConfigTemplateBuilder,
    surface::{Surface, WindowSurface},
};

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
    window::{Window, WindowId},
};

use glutin::{display::GetGlDisplay, prelude::*};

use glutin_winit::{DisplayBuilder, GlWindow};

use crate::{
    font_registry::FontRegistry,
    renderers::{self, TextRenderer},
    text_manager::{TermGlyph, TextManager},
};

use crate::{gl_handler::GlHandler, macros::macs::include_font};

use std::time::Instant;

// const TEXT: &str = "-<- <= b🤔";
// const TEXT: &str = "-<-<=_";
// const TEXT: &str = "  NO<=AL   -<-";
const TEXT: &str = " NORMAL  17:09:09  ";
pub struct App<'a> {
    gl_handler: GlHandler,
    state: Option<AppState>,
    gl: Option<Rc<glow::Context>>,
    font_registry: FontRegistry,
    text_renderer: Option<TextRenderer<'a>>,
    triangle_renderer: Option<renderers::TriangleRenderer>,
    quad_renderer: Option<renderers::QuadRenderer>,
    text_index: usize,
    conf_font_size_px: u32,
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
            text_index: 0,
            conf_font_size_px: 16,
        }
    }

    /// SAFETY: This function should only be called after the OpenGL context has been created and made current in the `resumed` method.
    /// Calling this function before that will result in undefined behavior.
    pub unsafe fn gl(&self) -> &glow::Context {
        self.gl.as_ref().unwrap()
    }

    pub fn on_resize(&mut self, size: &PhysicalSize<u32>) {
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
        } else {
            println!("Resize event received before GL surface was created, ignoring.");
        }

        println!("REDRAWING -----------------------------------V");
        unsafe {
            let font_size = self.text_renderer.as_ref().unwrap().font_size();

            println!(
                "!!!Cell size: {}x{}",
                font_size.0 as i32, font_size.1 as i32
            );

            self.quad_renderer = Some(renderers::QuadRenderer::new(
                self.gl.as_ref().unwrap().clone(),
                size.width as i32,
                size.height as i32,
            ));

            self.text_renderer
                .as_mut()
                .unwrap()
                .set_viewport(size.width as i32, size.height as i32);

            self.gl()
                .viewport(0, 0, size.width as i32, size.height as i32);

            let chars = &TEXT.chars().collect::<Vec<_>>();
            println!("Rendering text: {:?}", chars);

            let green = (87, 211, 109);
            let white = (217, 217, 217);
            let black = (0, 0, 0);

            let color_by_range: Vec<((u8, u8, u8), (u8, u8, u8))> = {
                // range, fg_color, bg_color
                // " NORMAL  17:09:09  ";
                let count = [
                    (1, green, black),  // Red for ""
                    (8, black, green),  // Red for " NORMAL "
                    (1, white, green),  // Green for ""
                    (12, black, white), // White for "  17:09:09  "
                    (1, white, black),  // White for ""
                ];

                let count_len = count
                    .iter()
                    .map(|i| i.0)
                    .reduce(|acc, c| acc + c)
                    .unwrap_or(0);

                let mut result = Vec::with_capacity(count_len);

                for (len, fg_color, bg_color) in count {
                    for _ in 0..len {
                        result.push((fg_color, bg_color));
                    }
                }

                result
            };

            let glyphs: Vec<TermGlyph> = chars
                .iter()
                .enumerate()
                .map(|(i, &c)| {
                    let (fg_color, bg_color) =
                        color_by_range.get(i).cloned().unwrap_or((white, black));
                    TermGlyph {
                        char: c,
                        fg_color,
                        bg_color,
                    }
                })
                .collect();

            self.quad_renderer.as_ref().unwrap().with(|| {
                let proj = ortho(size.width as f32, size.height as f32);

                let rows = self.text_renderer.as_ref().unwrap().text_manager.rows;

                for row in 0..rows {
                    if row % 2 == 0 && row != 0 {
                        continue;
                    }
                    self.text_renderer
                        .as_mut()
                        .unwrap()
                        .draw_glyphs(&glyphs, row, 0, &proj);
                }
            });
        }

        let duration = start.elapsed();
        println!("Resized in {} ms", duration.as_millis());
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

            let width = window.inner_size().width as i32;
            let height = window.inner_size().height as i32;
            TextRenderer::<'a>::new(
                self.gl.as_ref().unwrap().clone(),
                font_registry,
                self.conf_font_size_px,
                (width, height),
            )
        });

        self.quad_renderer.get_or_insert_with(|| unsafe {
            renderers::QuadRenderer::new(
                self.gl.as_ref().unwrap().clone(),
                window.inner_size().width as i32,
                window.inner_size().height as i32,
            )
        });

        let font_size = self.text_renderer.as_ref().unwrap().font_size();

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
                self.on_resize(&size);
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let start = Instant::now();
                if let Some(AppState {
                    gl_surface,
                    window: _,
                }) = &self.state
                {
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
            WindowEvent::KeyboardInput {
                device_id: _,
                event,
                is_synthetic: _,
            } => {
                //conf_font_size_px
                if event.repeat || event.state != winit::event::ElementState::Pressed {
                    return;
                }

                match event.physical_key {
                    winit::keyboard::PhysicalKey::Code(KeyCode::Equal) => {
                        self.conf_font_size_px += 1;
                        println!("Increasing font size to {}", self.conf_font_size_px);
                    }
                    winit::keyboard::PhysicalKey::Code(KeyCode::Minus) => {
                        if self.conf_font_size_px > 2 {
                            self.conf_font_size_px -= 1;
                            println!("Decreasing font size to {}", self.conf_font_size_px);
                        }
                    }
                    _ => {}
                }

                // for now, even it says px, it's actually font size in points, but we can change it later to be more intuitive

                if let Some(AppState { gl_surface, window }) = self.state.as_ref() {
                    self.text_renderer
                        .as_mut()
                        .unwrap()
                        .update_font_size(self.conf_font_size_px, 96);

                    let size = window.inner_size();
                    let physical_size = PhysicalSize::new(size.width, size.height);
                    window.request_redraw();
                    self.on_resize(&physical_size);
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
