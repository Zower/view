use std::ops::{Deref, DerefMut};

use app::App;
use bevy_reflect::{reflect_trait, GetPath, GetTypeRegistration, ParsedPath, Reflect};

mod app;
mod elements;
pub mod patch;
mod runner;
mod start;
mod text;

use cosmic_text::FontSystem;
pub use elements::*;

use crossbeam::channel::TryRecvError;
use femtovg::renderer::OpenGl;
use runner::Runner;

pub type Result<T> = miette::Result<T>;

pub type Point = taffy::Point<u32>;
pub type Size = taffy::Size<u32>;
pub type Rect = taffy::Rect<u32>;

pub struct Color(femtovg::Color);

pub fn run<V: View + GetTypeRegistration + GetPath>(v: V) -> crate::Result<()> {
    let (canvas, el, pcc, surface, window, _config) = start::create_event_loop(800, 600, "view");

    let app = App::new(v, window.inner_size());
    let cache = text::init_cache();

    Runner {
        el,
        canvas: Canvas {
            inner: canvas,
            text_cache: cache,
        },
        window: (surface, window),
        gl_context: pcc,
    }
    .run(app)
}

#[reflect_trait]
pub trait View: Reflect {
    fn build(&self) -> Element;
    fn messages(&mut self) {}
}

pub struct Canvas {
    inner: femtovg::Canvas<OpenGl>,
    pub text_cache: text::RenderCache,
}

impl Canvas {
    pub fn font_system(&mut self) -> &mut FontSystem {
        &mut self.text_cache.font_system
    }
}

impl Deref for Canvas {
    type Target = femtovg::Canvas<OpenGl>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Canvas {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[reflect_trait]
pub trait State {
    fn is_dirty(&self) -> bool;
}

#[derive(Reflect, Debug)]
#[reflect(State)]
pub struct Messages<M> {
    #[reflect(ignore)]
    inner: MessageInner<M>,
}

impl<M> State for Messages<M> {
    fn is_dirty(&self) -> bool {
        !self.inner.rx.is_empty()
    }
}

impl<M> Default for Messages<M> {
    fn default() -> Self {
        Self {
            inner: MessageInner::default(),
        }
    }
}

#[derive(Debug)]
pub struct MessageInner<M> {
    rx: crossbeam::channel::Receiver<M>,
    tx: crossbeam::channel::Sender<M>,
}

impl<M> Default for MessageInner<M> {
    fn default() -> Self {
        let (tx, rx) = crossbeam::channel::unbounded();
        Self { rx, tx }
    }
}

impl<M: Clone + 'static> Messages<M> {
    pub fn send(&self, message: M) -> Triggerable {
        let sender = self.inner.tx.clone();
        Triggerable {
            f: Box::new(move || {
                sender.send(message.clone()).expect("Failed to send");
            }),
        }
    }

    pub fn recv(&self) -> Option<M> {
        self.inner
            .rx
            .try_recv()
            .inspect_err(|f| {
                let TryRecvError::Empty = f else {
                    panic!("Closed channel")
                };
            })
            .ok()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Layout {
    /// The relative ordering of the node
    ///
    /// Nodes with a higher order should be rendered on top of those with a lower order.
    /// This is effectively a topological sort of each tree.
    pub order: u32,
    /// The top-left corner of the node
    pub location: Point,
    /// The width and height of the node
    pub size: Size,
    // #[cfg(feature = "content_size")]
    // /// The width and height of the content inside the node. This may be larger than the size of the node in the case of
    // /// overflowing content and is useful for computing a "scroll width/height" for scrollable nodes
    // pub content_size: Size<f32>,
    /// The size of the scrollbars in each dimension. If there is no scrollbar then the size will be zero.
    pub scrollbar_size: Size,
    /// The size of the borders of the node
    pub border: Rect,
    /// The size of the padding of the node
    pub padding: Rect,
}

impl Layout {
    pub fn plus_location(mut self, location: Point) -> Self {
        self.location = Point {
            x: self.location.x + location.x,
            y: self.location.y + location.y,
        };

        self
    }
}

impl From<taffy::Layout> for Layout {
    fn from(value: taffy::Layout) -> Self {
        fn map_size(p: taffy::Size<f32>) -> Size {
            Size {
                width: p.width as u32,
                height: p.height as u32,
            }
        }

        fn map_rect(p: taffy::Rect<f32>) -> Rect {
            Rect {
                left: p.left as u32,
                right: p.right as u32,
                top: p.top as u32,
                bottom: p.bottom as u32,
            }
        }

        Self {
            order: value.order,
            location: Point {
                x: value.location.x as u32,
                y: value.location.y as u32,
            },
            size: map_size(value.size),
            scrollbar_size: map_size(value.scrollbar_size),
            border: map_rect(value.border),
            padding: map_rect(value.padding),
        }
    }
}

pub struct Triggerable {
    f: Box<dyn Fn()>,
}

impl Triggerable {
    pub fn trigger(&self) {
        (self.f)()
    }
}

pub enum GlobalEvent {}

impl Color {
    pub fn rgb(r: u8, b: u8, g: u8) -> Self {
        Self(femtovg::Color::rgb(r, g, b))
    }

    pub fn rgba(r: u8, b: u8, g: u8, a: u8) -> Self {
        Self(femtovg::Color::rgba(r, g, b, a))
    }
}

impl Default for Color {
    fn default() -> Self {
        Self(femtovg::Color::white())
    }
}

impl From<Color> for cosmic_text::Color {
    fn from(value: Color) -> Self {
        cosmic_text::Color::rgba(
            (value.0.r * 255.) as u8,
            (value.0.g * 255.) as u8,
            (value.0.b * 255.) as u8,
            (value.0.a * 255.) as u8,
        )
    }
}

impl From<Color> for femtovg::Color {
    fn from(value: Color) -> Self {
        value.0
    }
}

#[macro_export]
macro_rules! view {
    ($view:expr) => {{
        fn assert_view<V: View>(v: &V) {}

        let _ = &$view;
        assert_view($view);

        ::bevy_reflect::ParsedPath::parse_static(::const_format::str_replace!(
            stringify!($view),
            "&self",
            ""
        ))
        .unwrap()
    }};
}
