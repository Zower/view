use std::thread;

use crossbeam::channel::Select;
use glutin::{prelude::PossiblyCurrentGlContext, surface::GlSurface};
use imgref::Img;
use miette::IntoDiagnostic;
use rgb::RGBA8;
use tungstenite::util::NonBlockingResult;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
};

use crate::{
    app::AppEvent,
    runner::{WindowData, Windows},
    Canvas, ClientMessage, Frame, Point, ServerMessage, SurrogateMessage,
};

#[doc(hidden)]
struct SurrogateRunner {
    canvas: Canvas,
    windows: Windows,
    gl_context: glutin::context::PossiblyCurrentContext,
    sender: crossbeam::channel::Sender<ServerMessage>,
    last_frame: Option<Frame>,
}

pub fn run() -> miette::Result<()> {
    let server = std::net::TcpListener::bind("127.0.0.1:9001").into_diagnostic()?;

    let (sender, receiver) = crossbeam::channel::unbounded::<ServerMessage>();

    let (canvas, el, pcc, surface, window, _config) =
        crate::start::create_event_loop(800, 600, "view");

    let proxy = el.create_proxy();

    thread::spawn::<_, miette::Result<()>>(move || 'accept: loop {
        let (stream, _) = server.accept().into_diagnostic()?;
        let mut socket = tungstenite::accept(stream).into_diagnostic()?;

        dbg!("New connection");

        loop {
            match socket.read() {
                Ok(message) => {
                    let SurrogateMessage::FromClient(message) = (match message {
                        tungstenite::Message::Binary(message) => {
                            bincode::deserialize::<SurrogateMessage>(&message).into_diagnostic()?
                        }
                        tungstenite::Message::Close(_) => continue,
                        msg => {
                            dbg!(msg);
                            continue;
                        }
                    }) else {
                        panic!()
                    };

                    dbg!(&message);
                    proxy.send_event(message).unwrap();
                }
                Err(tungstenite::Error::ConnectionClosed) => {
                    panic!();
                    dbg!("Closed, continue");

                    continue 'accept;
                }
                Err(err) => {
                    panic!();
                    Err(err).into_diagnostic()?
                }
            }
        }
    });

    thread::spawn(move || {
        // crossbeam::select! {
        //     recv(receiver) -> msg => { dbg!(msg.unwrap()); }
        //     recv(receive_message_receiver) -> msg => { () }
        // }
    });

    window.set_visible(true);

    let cache = crate::text::init_cache();
    // let app = App::new(v, window.inner_size());
    // let cache = text::init_cache();

    el.run_app(&mut SurrogateRunner {
        canvas: Canvas {
            inner: canvas,
            text_cache: cache,
        },
        windows: Windows::new(window, surface),
        gl_context: pcc,
        sender,
        last_frame: None,
    })
    .into_diagnostic()
}

impl ApplicationHandler<ClientMessage> for SurrogateRunner {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let Self {
            ref mut canvas,
            windows,
            gl_context,
            sender,
            last_frame,
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
                sender
                    .send(ServerMessage::AppEvent(AppEvent::Paint(
                        window.inner_size(),
                    )))
                    .unwrap();

                gl_context
                    .make_current(&surface)
                    .expect("Making current to work");

                if let Some(last_frame) = last_frame.take() {
                    let data = Img::new(last_frame.data, last_frame.width, last_frame.height);

                    let image = canvas
                        .create_image(data.as_ref(), femtovg::ImageFlags::empty())
                        .unwrap();

                    let fill_paint = femtovg::Paint::image(
                        image,
                        0.0,
                        0.0,
                        window.inner_size().width as f32,
                        window.inner_size().height as f32,
                        0.0,
                        1.0,
                    );

                    let mut path = femtovg::Path::new();

                    path.rect(0.0, 0.0, last_frame.width as f32, last_frame.height as f32);
                    canvas.fill_path(&path, &fill_paint);

                    canvas.delete_image(image);
                }

                canvas.flush();

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
                // app.event(AppEvent::Clicked(mouse_pos.x, mouse_pos.y), canvas);

                window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let _pixels = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, delta) => -delta * 45.,
                    // TODO probably invert this too
                    winit::event::MouseScrollDelta::PixelDelta(delta) => delta.y as f32,
                };
            }
            WindowEvent::KeyboardInput { event, .. } => {
                dbg!(event);
            }
            WindowEvent::Resized(size) => {
                canvas.set_size(size.width, size.height, window.scale_factor() as f32);
                window.request_redraw();
            }
            _ => {}
        }
    }

    fn user_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        event: ClientMessage,
    ) {
        match event {
            ClientMessage::Update(msg) => {
                dbg!(msg);
            }
            ClientMessage::Frame(frame) => {
                self.last_frame = Some(frame);

                for (_, window) in self.windows.iter() {
                    window.window.request_redraw();
                }
            }
        }
    }
}
