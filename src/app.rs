use std::{ffi::CString, num::NonZeroU32};

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

use crate::renderers;

use crate::{gl_handler::GlHandler, macros::macs::include_font};

pub struct App {
    gl_handler: GlHandler,
    state: Option<AppState>,
    gl: Option<glow::Context>,
    text_renderer: Option<renderers::TextRenderer>,
    triangle_renderer: Option<renderers::TriangleRenderer>,
}

struct AppState {
    gl_surface: Surface<WindowSurface>,
    window: Window,
}

impl App {
    pub fn new(template: ConfigTemplateBuilder, display_builder: DisplayBuilder) -> Self {
        Self {
            gl_handler: GlHandler::new(template, display_builder),
            state: None,
            gl: None,
            text_renderer: None,
            triangle_renderer: None,
        }
    }
}

impl ApplicationHandler for App {
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

        self.gl = Some(gl);

        self.text_renderer.get_or_insert_with(|| unsafe {
            renderers::TextRenderer::new(
                self.gl.as_ref().unwrap(),
                include_font!("CaskaydiaCoveNerdFont-Regular.ttf"),
                48,
            )
        });

        self.triangle_renderer.get_or_insert_with(|| unsafe {
            renderers::TriangleRenderer::new(self.gl.as_ref().unwrap())
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
                println!("Window resized to {}x{}", size.width, size.height);
                if let Some(AppState {
                    gl_surface,
                    window: _,
                }) = self.state.as_ref()
                {
                    let gl_context = self.gl_handler.gl_context.as_ref().unwrap();
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

                        self.triangle_renderer = Some(
                            self.triangle_renderer
                                .take()
                                .unwrap()
                                .resize(self.gl.as_ref().unwrap()),
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
                if let Some(AppState { gl_surface, window }) = &self.state {
                    let proj = ortho(
                        window.inner_size().width as f32,
                        window.inner_size().height as f32,
                    );

                    let gl_context = self.gl_handler.gl_context.as_ref().unwrap();

                    let triangles_positions = [(0.1, 0.0), (-0.5, -0.5), (0.5, -0.5), (0.0, 0.0)];
                    let text_positions = [(0.0, 100.0), (0.0, 200.0), (0.0, 300.0), (0.0, 400.0)];

                    unsafe {
                        for point in triangles_positions {
                            self.triangle_renderer.as_ref().unwrap().render(
                                self.gl.as_ref().unwrap(),
                                point,
                                0.1,
                                (1.0, 1.0, 1.0),
                            );
                        }

                        for point in text_positions {
                            self.text_renderer.as_ref().unwrap().draw_text(
                                self.gl.as_ref().unwrap(),
                                "office != affine -> ligatures?",
                                point.0,
                                point.1,
                                48.0,
                                [1.0, 1.0, 0.0],
                                &proj,
                            );
                        }
                    }

                    gl_surface.swap_buffers(gl_context).unwrap();
                    window.request_redraw();
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
