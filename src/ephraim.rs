// ephraim because it's similar to eframe but custom-made for this specific
// project.

use egui_glow::{painter::Context, EguiGlow};
use glutin::{event_loop::EventLoop, WindowedContext, PossiblyCurrent};
use std::{error::Error, rc::Rc};

pub trait App {
    fn update(&mut self, ctx: &egui::Context);
    fn quit(&self) -> bool;
}

pub struct AppWindow {
	eguiglow: EguiGlow,
	el: EventLoop<()>,
	cb: WindowedContext<PossiblyCurrent>,
	app: Box<dyn App>,
}

impl AppWindow {
	pub fn new(app: Box<dyn App>) -> Result<AppWindow, Box<dyn Error>> {
		use glutin::{Api, GlRequest, dpi::{Size, LogicalSize}};
		let el = glutin::event_loop::EventLoop::new();
		let wb = glutin::window::WindowBuilder::new()
			.with_title("Doom 64 CE launcher")
			.with_inner_size(Size::new(LogicalSize::new(550.0f32, 300.0f32)));
		let cb = glutin::ContextBuilder::new()
			.with_gl(GlRequest::Specific(Api::OpenGl, (3, 3)))
			.build_windowed(wb, &el)?;
		let cb = unsafe { cb.make_current() }.map_err(|(_old_ctx, err)| err)?;
		let ctx = Rc::from(unsafe {
			Context::from_loader_function(|name| cb.get_proc_address(name)) });
		let eguiglow = EguiGlow::new(cb.window(), Rc::clone(&ctx));
		Ok(AppWindow {
			eguiglow, el, cb, app
		})
	}
	pub fn run(mut self) -> ! {
		self.el.run(move |event, _, control_flow| {
			// Some code copied from https://github.com/emilk/egui/blob/master/egui_glow/examples/pure_glow.rs
			let mut redraw = || {
				use glutin::event_loop::ControlFlow;
				let needs_repaint = self.eguiglow.run(self.cb.window(), |ctx| self.app.update(ctx));
				let quit = self.app.quit();

				*control_flow = if quit {
					ControlFlow::Exit
				} else if needs_repaint {
					self.cb.window().request_redraw();
					ControlFlow::Poll
				} else {
					ControlFlow::Wait
				};
	
				self.eguiglow.paint(self.cb.window());
				self.cb.swap_buffers().unwrap();
			};
	
			// Copied from https://github.com/emilk/egui/blob/master/egui_glow/examples/pure_glow.rs
			match event {
				// Platform-dependent event handlers to workaround a winit bug
				// See: https://github.com/rust-windowing/winit/issues/987
				// See: https://github.com/rust-windowing/winit/issues/1619
				glutin::event::Event::RedrawEventsCleared if cfg!(windows) => redraw(),
				glutin::event::Event::RedrawRequested(_) if !cfg!(windows) => redraw(),
	
				glutin::event::Event::WindowEvent { event, .. } => {
					use glutin::event::WindowEvent;
					if matches!(event, WindowEvent::CloseRequested | WindowEvent::Destroyed) {
						*control_flow = glutin::event_loop::ControlFlow::Exit;
					}
	
					if let glutin::event::WindowEvent::Resized(physical_size) = &event {
						self.cb.resize(*physical_size);
					} else if let glutin::event::WindowEvent::ScaleFactorChanged {
						new_inner_size,
						..
					} = &event
					{
						self.cb.resize(**new_inner_size);
					}
	
					self.eguiglow.on_event(&event);
	
					self.cb.window().request_redraw(); // TODO: ask egui if the events warrants a repaint instead
				}
				glutin::event::Event::LoopDestroyed => {
					self.eguiglow.destroy();
				}
	
				_ => (),
			}
		});
	}
}
