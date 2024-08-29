use std::thread;

use async_std::net::TcpListener;
use async_tungstenite::{accept_async, async_std::connect_async, tungstenite};
use futures::{SinkExt, StreamExt};
use glutin::{prelude::PossiblyCurrentGlContext, surface::GlSurface};
use imgref::Img;
use miette::IntoDiagnostic;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::EventLoopProxy,
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
    sender: async_std::channel::Sender<ServerMessage>,
    last_frame: Option<Frame>,
}

pub fn run() -> miette::Result<()> {
    // let server = std::net::TcpListener::bind("127.0.0.1:9001").into_diagnostic()?;

    let (canvas, el, pcc, surface, window, _config) =
        crate::start::create_event_loop(800, 600, "view");

    let proxy = el.create_proxy();

    let (server_message_sender, server_message_receiver) =
        async_std::channel::unbounded::<ServerMessage>();

    thread::spawn::<_, miette::Result<()>>(move || {
        async_std::task::block_on(run_internal(proxy, server_message_receiver))
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
        sender: server_message_sender,
        last_frame: None,
    })
    .into_diagnostic()
}

async fn run_internal(
    proxy: EventLoopProxy<ClientMessage>,
    receiver: async_std::channel::Receiver<ServerMessage>,
) -> miette::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9001")
        .await
        .into_diagnostic()?;

    dbg!("New connection");

    // Let's spawn the handling of each connection in a separate task.
    while let Ok((stream, _)) = listener.accept().await {
        let proxy = proxy.clone();
        let receiver = receiver.clone();
        let ws_stream = async_tungstenite::accept_async(stream)
            .await
            .expect("Error during the websocket handshake occurred");

        let (mut write, read) = ws_stream.split();

        async_std::task::spawn(async move {
            loop {
                let message = receiver.recv().await.unwrap();
                write
                    .send(tungstenite::Message::Binary(
                        bincode::serialize(&SurrogateMessage::FromServer(message)).unwrap(),
                    ))
                    .await
                    .unwrap();
            }
        });

        async_std::task::spawn(async move {
            read.for_each(|message| {
                match message {
                    Ok(message) => {
                        let SurrogateMessage::FromClient(message) = (match message {
                            tungstenite::Message::Binary(message) => {
                                bincode::deserialize::<SurrogateMessage>(&message).unwrap()
                            }
                            tungstenite::Message::Close(_) => return futures::future::ready(()),
                            msg => {
                                dbg!(msg);
                                return futures::future::ready(());
                            }
                        }) else {
                            panic!()
                        };

                        // dbg!(&message);
                        proxy.send_event(message).unwrap();
                    }
                    Err(tungstenite::Error::ConnectionClosed) => {
                        panic!();
                        dbg!("Closed, continue");
                    }
                    Err(err) => {
                        panic!();
                        // Err(err).into_diagnostic()?
                    }
                };

                futures::future::ready(())
            })
            .await;
        })
        .await;
    }

    Ok(())
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
                    .send_blocking(ServerMessage::AppEvent(AppEvent::Paint(
                        window.inner_size(),
                    )))
                    .unwrap();

                gl_context
                    .make_current(&surface)
                    .expect("Making current to work");

                dbg!(&last_frame.is_some());

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

                    // canvas.delete_image(image);
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
                sender
                    .send_blocking(ServerMessage::AppEvent(AppEvent::Clicked(
                        mouse_pos.x,
                        mouse_pos.y,
                    )))
                    .unwrap();
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
