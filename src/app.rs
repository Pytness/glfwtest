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
    renderers::{self, TextRenderer},
    text_manager::TextManager,
};

use crate::{gl_handler::GlHandler, macros::macs::include_font};

const FONT_SIZE: u32 = 32;

pub struct App {
    gl_handler: GlHandler,
    state: Option<AppState>,
    gl: Option<Rc<glow::Context>>,
    framebuffer: Option<glow::Framebuffer>,
    framebuffer_texture: Option<glow::NativeTexture>,
    text_renderer: Option<TextRenderer>,
    triangle_renderer: Option<renderers::TriangleRenderer>,
    quad_renderer: Option<renderers::QuadRenderer>,
    text_manager: Option<TextManager>,
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
            framebuffer: None,
            text_renderer: None,
            triangle_renderer: None,
            framebuffer_texture: None,
            quad_renderer: None,
            text_manager: None,
        }
    }

    /// SAFETY: This function should only be called after the OpenGL context has been created and made current in the `resumed` method.
    /// Calling this function before that will result in undefined behavior.
    pub unsafe fn gl(&self) -> &glow::Context {
        self.gl.as_ref().unwrap()
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

        self.gl = Some(Rc::new(gl));

        self.triangle_renderer.get_or_insert_with(|| unsafe {
            renderers::TriangleRenderer::new(self.gl.as_ref().unwrap().clone())
        });
        self.text_renderer.get_or_insert_with(|| unsafe {
            TextRenderer::new(
                self.gl.as_ref().unwrap().clone(),
                include_font!("CaskaydiaCoveNerdFont-Regular.ttf"),
                FONT_SIZE,
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
                        let font_size = self.text_renderer.as_ref().unwrap().font_size();
                        let cell_box = self.text_manager.as_ref().unwrap().get_cell_box(0, 0);
                        println!("Font size: {}x{}", font_size.0, font_size.1);
                        println!(
                            "Cell box for (0, 0): x={}, y={}, width={}, height={}",
                            cell_box.0, cell_box.1, cell_box.2, cell_box.3
                        );

                        self.text_manager.get_or_insert_with(|| {
                            TextManager::new(
                                font_size.0 as i32,
                                font_size.1 as i32,
                                size.width as i32,
                                size.height as i32,
                            )
                        });

                        self.quad_renderer = Some(renderers::QuadRenderer::new(
                            self.gl.as_ref().unwrap().clone(),
                            size.width as i32,
                            size.height as i32,
                        ));

                        self.gl()
                            .viewport(0, 0, size.width as i32, size.height as i32);
                        let proj = ortho(size.width as f32, size.height as f32);
                        self.quad_renderer.as_ref().unwrap().with(|| {
                            self.text_renderer.as_ref().unwrap().draw_text(
                                "office != affine - > --> -> ligatures?",
                                0.0,
                                0.0,
                                FONT_SIZE as f32,
                                [1.0, 1.0, 0.0],
                                &proj,
                            );
                        });
                    }
                }
            }
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(AppState { gl_surface, window }) = &self.state {
                    let gl_context = self.gl_handler.gl_context.as_ref().unwrap();

                    println!("Redrawing the application");

                    let triangles_positions = [(0.1, 0.0), (-0.5, -0.5), (0.5, -0.5), (0.0, 0.0)];

                    unsafe {
                        // self.quad_renderer.as_ref().unwrap().with(|| {
                        //     for point in triangles_positions {
                        //         self.triangle_renderer.as_ref().unwrap().render(
                        //             point,
                        //             0.5,
                        //             (1.0, 0.0, 0.0),
                        //         );
                        //     }
                        // });
                        let width = window.inner_size().width as i32;
                        let height = window.inner_size().height as i32;

                        let points = [(0, 0), (0, 2), (1, 1), (1, 3)];
                        for point in points {
                            let cell_box = self
                                .text_manager
                                .as_ref()
                                .unwrap()
                                .get_cell_box(point.0, point.1);
                            self.quad_renderer.as_ref().unwrap().clear_section(
                                cell_box.0,
                                height - cell_box.1 - cell_box.3,
                                cell_box.2,
                                cell_box.3,
                            );
                        }

                        self.quad_renderer.as_ref().unwrap().render();
                    }

                    gl_surface.swap_buffers(gl_context).unwrap();
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
