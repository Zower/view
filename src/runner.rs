use std::time::Instant;

use glutin::{prelude::PossiblyCurrentGlContext, surface::GlSurface};
use miette::IntoDiagnostic;
use winit::{
    event::{ElementState, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

use crate::{
    app::{App, AppEvent},
    Canvas, GlobalEvent, Point, View,
};

pub struct Runner {
    pub el: EventLoop<GlobalEvent>,
    pub(crate) canvas: Canvas,
    pub(crate) window: (
        glutin::surface::Surface<glutin::surface::WindowSurface>,
        winit::window::Window,
    ),
    pub(crate) gl_context: glutin::context::PossiblyCurrentContext,
}

impl Runner {
    pub fn run<V: View>(self, mut app: App<V>) -> crate::Result<()> {
        let Self {
            el,
            mut canvas,
            window: (initial_surface, initial_window),
            gl_context,
        } = self;

        Self::init(
            // canvas,
            // gl_context,
            // initial_surface,
            &initial_window,
            // crate::Proxy(el.create_proxy()),
        )?;

        // app.startup();
        // app.logic();

        let mut mouse_pos = Point { x: 0, y: 0 };

        el.run(move |event, target| {
            match event {
                winit::event::Event::NewEvents(StartCause::Init) => {
                    target.set_control_flow(ControlFlow::Wait);
                }
                winit::event::Event::NewEvents(StartCause::Poll) => {
                    let now = std::time::Instant::now();
                    // app.main();
                    let elapsed = now.elapsed();

                    println!("{:?}", elapsed);
                    target.set_control_flow(ControlFlow::Wait);
                }
                winit::event::Event::UserEvent(event) => match event {
                    // FlareEvent::LspEvent(event) => {
                    //     app.event(LspEvent(event));

                    //     target.set_control_flow(ControlFlow::Poll);
                    // }
                },
                winit::event::Event::WindowEvent {
                    event,
                    window_id: _window_id,
                } => {
                    match event {
                        WindowEvent::RedrawRequested => {
                            gl_context
                                .make_current(&initial_surface)
                                .expect("Making current to work");

                            canvas.clear_rect(
                                0,
                                0,
                                initial_window.inner_size().width,
                                initial_window.inner_size().height,
                                femtovg::Color::black(),
                            );

                            app.paint(initial_window.inner_size(), &mut canvas);

                            canvas.flush();

                            initial_surface
                                .swap_buffers(&gl_context)
                                .expect("Swapping buffer to work");
                        }

                        WindowEvent::CloseRequested => target.exit(),
                        WindowEvent::ModifiersChanged(_modifiers) => {}
                        WindowEvent::CursorMoved { position, .. } => {
                            mouse_pos = Point {
                                x: position.x as u32,
                                y: position.y as u32,
                            };
                        }
                        WindowEvent::MouseInput {
                            state: ElementState::Pressed,
                            ..
                        } => {
                            let now = Instant::now();
                            app.event(AppEvent::Clicked(mouse_pos.x, mouse_pos.y));
                            dbg!(now.elapsed());
                            // warn!("Unused mouse input");

                            // let window = &windows[&window_id];

                            initial_window.request_redraw();
                        }
                        WindowEvent::MouseWheel { delta, .. } => {
                            let _pixels = match delta {
                                winit::event::MouseScrollDelta::LineDelta(_, delta) => -delta * 45.,
                                // TODO probably invert this too
                                winit::event::MouseScrollDelta::PixelDelta(delta) => delta.y as f32,
                            };

                            // app.event(Scrolled {
                            //     pixels,
                            //     position: mouse_pos,
                            // });

                            // app.main();
                        }
                        WindowEvent::KeyboardInput { event, .. } => {
                            dbg!(event);
                            // info!("Received keyboard event");
                            // app.event(KeyEvent(event.into()));
                            // app.main();
                        }
                        WindowEvent::Resized(size) => {
                            dbg!(size);
                            // app.event(Resize(window_id, size));
                            // app.main();
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        })
        .into_diagnostic()?;

        Ok(())
    }

    fn init(
        // canvas: Canvas,
        // gl_context: PossiblyCurrentContext,
        // initial_surface: Surface<WindowSurface>,
        initial_window: &winit::window::Window,
        // el_proxy: crate::Proxy,
    ) -> crate::Result<()> {
        // let world = &mut app.world;
        // let mut surfaces = Surfaces::default();

        initial_window.set_visible(true);
        // let window_id = world
        //     .spawn(Window {
        //         inner: initial_window,
        //     })
        //     .id();

        // surfaces.insert(window_id, initial_surface);

        // world.insert_non_send_resource(RenderContext {
        //     gl_context,
        //     canvas,
        //     render_cache: text::init_cache(),
        //     surfaces,
        // });

        // app.resource(config::config()?);
        // app.resource(Editor::new());
        // app.resource(ProxyResource(el_proxy));

        // app.initialize_event::<Resize>();
        // app.initialize_event::<KeyEvent>();
        // app.initialize_event::<Scrolled>();
        // app.initialize_event::<BufferAction>();
        // app.initialize_event::<LspEvent>();

        // app.inititalize(EditorModule);
        // app.inititalize(UiModule);

        Ok(())
    }
}
