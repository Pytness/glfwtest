mod ansi_parser;
mod app;
mod font_registry;
mod gl_handler;
mod macros;
mod renderers;
mod text_manager;

use glutin::config::ConfigTemplateBuilder;

use winit::event_loop::EventLoop;

use glutin_winit::DisplayBuilder;

use app::App;
use gl_handler::window_attributes;

fn main() {
    let event_loop = EventLoop::new().unwrap();

    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_transparency(true);

    let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes()));

    let mut app = App::new(template, display_builder);
    match event_loop.run_app(&mut app) {
        Ok(()) => (),
        Err(e) => eprintln!("Application error: {e}"),
    };
}
