use std::{collections::HashMap, time::Instant};

use glutin::{prelude::PossiblyCurrentGlContext, surface::GlSurface};
use miette::IntoDiagnostic;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::EventLoop,
    window::WindowId,
};

use crate::{
    app::{App, AppEvent},
    Canvas, GlobalEvent, Point,
};

pub(crate) struct Runner {
    pub(crate) app: App,
    pub(crate) canvas: Canvas,
    pub(crate) windows: Windows,
    pub(crate) gl_context: glutin::context::PossiblyCurrentContext,
}

impl Runner {
    pub fn run(mut self, el: EventLoop<GlobalEvent>) -> crate::Result<()> {
        Self::init(&self.windows.root())?;

        el.run_app(&mut self).into_diagnostic()?;

        Ok(())
    }

    fn init(initial_window: &winit::window::Window) -> crate::Result<()> {
        initial_window.set_visible(true);

        Ok(())
    }
}

impl ApplicationHandler<GlobalEvent> for Runner {
    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Self {
            app,
            ref mut canvas,
            windows,
            gl_context,
        } = self;

        let Some(WindowData {
            window,
            surface,
            mouse_pos,
            parent: _,
        }) = windows.get_mut(&window_id)
        else {
            dbg!("Missing window");
            return;
        };

        match event {
            WindowEvent::RedrawRequested => {
                gl_context
                    .make_current(&surface)
                    .expect("Making current to work");
                canvas.inner.clear_rect(
                    0,
                    0,
                    window.inner_size().width,
                    window.inner_size().height,
                    femtovg::Color::black(),
                );

                app.event(AppEvent::Paint(window.inner_size()), canvas);

                canvas.inner.flush();

                surface
                    .swap_buffers(&gl_context)
                    .expect("Swapping buffer to work");
            }

            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ModifiersChanged(_modifiers) => {}
            WindowEvent::CursorMoved { position, .. } => {
                *mouse_pos = Point {
                    x: position.x as u32,
                    y: position.y as u32,
                };
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            } => {
                let now = Instant::now();
                app.event(AppEvent::Clicked(mouse_pos.x, mouse_pos.y), canvas);
                let elapsed = now.elapsed();
                dbg!(elapsed);

                window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let _pixels = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, delta) => -delta * 45.,
                    // TODO probably invert this too
                    winit::event::MouseScrollDelta::PixelDelta(delta) => delta.y as f32,
                };

                // app.main();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                app.event(AppEvent::Key(event), canvas);
                window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                app.event(AppEvent::Resize(size), canvas);
                canvas
                    .inner
                    .set_size(size.width, size.height, window.scale_factor() as f32);
                window.request_redraw();
            }
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, event: GlobalEvent) {
        match event {
            GlobalEvent::Dirty { hint } => {
                self.app.hint_dirty(hint);
            } // FlareEvent::LspEvent(event) => {
              //     app.event(LspEvent(event));

              //     target.set_control_flow(ControlFlow::Poll);
              // }
        }
    }
}

pub(crate) struct Windows {
    root: WindowId,
    map: HashMap<WindowId, WindowData>,
}

impl Windows {
    pub fn new(
        window: winit::window::Window,
        surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
    ) -> Self {
        let id = window.id();
        let window_data = WindowData {
            window,
            surface,
            mouse_pos: Point { x: 0, y: 0 },
            parent: None,
        };

        Self {
            root: id,
            map: HashMap::from([(id, window_data)]),
        }
    }
    pub fn root(&self) -> &winit::window::Window {
        &self.map[&self.root].window
    }

    pub fn iter(&self) -> impl Iterator<Item = (&WindowId, &WindowData)> {
        self.map.iter()
    }

    pub fn get_mut(&mut self, id: &WindowId) -> Option<&mut WindowData> {
        self.map.get_mut(id)
    }
}

pub(crate) struct WindowData {
    pub(crate) window: winit::window::Window,
    pub(crate) surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
    pub(crate) mouse_pos: Point,
    pub(crate) parent: Option<WindowId>,
}
