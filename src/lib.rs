use std::ops::{Deref, DerefMut};

use app::App;
use bevy_reflect::{reflect_trait, GetPath, GetTypeRegistration, Reflect, TypeRegistry};

mod app;
mod elements;
pub mod patch;
mod runner;
mod start;
mod text;
mod utils;

use taffy::NodeId;
pub use utils::*;

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

struct IntoElementView<T>(T);

impl<T: View> From<IntoElementView<T>> for MountableElement {
    fn from(value: IntoElementView<T>) -> Self {
        MountableElement::View(ViewElement(Box::new(value.0)))
    }
}

impl<T: View> Element for T {
    type Children = TodoRemoveElementWithChildrenVec;
    fn consume(self) -> (IntoElementView<Self>, Self::Children) {
        todo!()
        // (
        //     MountableElement::View(ViewElement(Box::new(self))),
        //     self.build().become_element(),
        // )
    }

    fn convert(
        children: Self::Children,
        registry: &mut TypeRegistry,
        tree: &mut app::ElementTree,
        parent: NodeId,
        idx: Option<usize>,
    ) {
        todo!()
    }
}

pub trait Element {
    type Children;

    fn consume(self) -> (impl Into<MountableElement>, Self::Children);
    fn convert(
        children: Self::Children,
        registry: &mut TypeRegistry,
        tree: &mut app::ElementTree,
        parent: NodeId,
        idx: Option<usize>,
    );
}

// impl<T: Into<MountableElement>> BecomeElement for T {
//     fn into(self) -> impl MountedElementBehaviour {
//         self.into()
//     }
// }

pub trait View: Register {
    fn build(&self) -> impl Element
    where
        Self: Sized;
}

pub trait Register: Reflect {
    fn register(&self, registry: &mut TypeRegistry);
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
pub(crate) trait StateTrait {
    fn is_dirty(&self) -> bool;
    fn init(&mut self);
    fn reuse(&mut self, other: &mut dyn Reflect);
    fn process(&mut self);
}

pub trait Receiver {
    type Message;

    fn reduce(&mut self, message: Self::Message);
}

#[derive(Reflect, Debug, Clone)]
#[reflect(StateTrait)]
pub struct State<M: Clone + 'static, S: Receiver<Message = M> + 'static> {
    #[reflect(ignore)]
    state: Option<S>,
    #[reflect(ignore)]
    inner: MessageInner<M>,
    #[reflect(ignore)]
    #[reflect(default = "create_state_fake")]
    create_state: fn() -> S,
}

pub(crate) trait Message: Clone + 'static {}

impl<T: Clone + 'static> Message for T {}

fn create_state_fake<S>() -> fn() -> S {
    panic!()
}

impl<M: Message, S: Receiver<Message = M> + 'static> StateTrait for State<M, S> {
    fn is_dirty(&self) -> bool {
        !self.inner.rx.is_empty()
    }

    fn process(&mut self) {
        while let Some(message) = self.recv() {
            self.deref_mut().reduce(message);
        }
    }

    fn init(&mut self) {
        self.state = Some((self.create_state)());
    }

    fn reuse(&mut self, other: &mut dyn Reflect) {
        let selfy = other.as_any_mut().downcast_mut::<Self>().unwrap();

        std::mem::swap(&mut self.state, &mut selfy.state);
    }
}

impl<M: Message, S: Receiver<Message = M> + 'static> Deref for State<M, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        self.state.as_ref().unwrap()
    }
}

impl<M: Message, S: Receiver<Message = M> + 'static> DerefMut for State<M, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.state.as_mut().unwrap()
    }
}

impl<M: Message, S: Default + Receiver<Message = M> + 'static> Default for State<M, S> {
    fn default() -> Self {
        Self {
            inner: MessageInner::default(),
            state: None,
            create_state: Default::default,
        }
    }
}

#[derive(Debug, Clone)]
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

impl<M: Clone + 'static, S: Receiver<Message = M>> State<M, S> {
    pub fn create_state(f: fn() -> S) -> Self {
        Self {
            inner: MessageInner::default(),
            state: None,
            create_state: f,
        }
    }

    pub fn then_send(&self, message: M) -> Triggerable {
        let sender = self.inner.tx.clone();
        Triggerable {
            f: Box::new(move || {
                if let Err(err) = sender.send(message.clone()) {
                    dbg!("WARN: ", err);
                }
            }),
        }
    }

    fn recv(&self) -> Option<M> {
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

pub trait IntoElement {
    fn element(self) -> TodoRemoveElementWithChildrenVec;
}

impl<T: Into<TodoRemoveElementWithChildrenVec>> IntoElement for T {
    fn element(self) -> TodoRemoveElementWithChildrenVec {
        self.into()
    }
}
