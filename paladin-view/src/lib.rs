use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use app::{App, AppEvent};
use bevy_reflect::{reflect_trait, Reflect, TypeRegistry};

pub mod app;
mod elements;
pub mod patch;
pub mod prelude;
mod runner;

mod start;
mod text;

#[doc(hidden)]
pub mod surrogate;

mod utils;

pub mod reflect {
    pub use bevy_reflect::*;
}

pub mod taffy {
    pub use taffy::*;
}

use miette::IntoDiagnostic;
use rgb::RGBA8;
use serde::{Deserialize, Serialize};
use taffy::NodeId;
pub use utils::*;

use cosmic_text::FontSystem;
pub use elements::*;

use crossbeam::channel::TryRecvError;
use femtovg::renderer::OpenGl;
use runner::{Runner, Windows};

pub type Result<T> = miette::Result<T>;

// Some utility types
pub type Point = taffy::Point<u32>;
pub type Size = taffy::Size<u32>;
pub type Rect = taffy::Rect<u32>;
pub struct Color(femtovg::Color);

use winit::dpi::PhysicalSize;

/// Run the app.
/// Call this once with your top level view.
pub fn run<V: View>(v: V) -> crate::Result<()> {
    let (canvas, el, pcc, surface, window, _config) = start::create_event_loop(800, 600, "view");

    let canvas = Canvas {
        inner: canvas,
        text_cache: text::init_cache(),
    };

    let initial_state;
    #[cfg(debug_assertions)]
    {
        let (mut socket, _response) =
            tungstenite::connect("ws://localhost:9001/socket").into_diagnostic()?;

        socket
            .write(tungstenite::Message::Binary(
                bincode::serialize(&ClientMessage::RequestState).into_diagnostic()?,
            ))
            .into_diagnostic()?;

        socket.flush().into_diagnostic()?;

        let tungstenite::Message::Binary(binary) = socket.read().into_diagnostic()? else {
            panic!();
        };

        let message = bincode::deserialize::<ServerMessage>(&binary).into_diagnostic()?;

        initial_state = match message {
            ServerMessage::State(state) => state,
            ServerMessage::NoState => String::new(),
        }
    };
    #[cfg(not(debug_assertions))]
    {
        initial_state = String::new()
    }

    dbg!(initial_state);

    let app = App::new(v, PhysicalSize::new(300, 400));

    Runner {
        app,
        windows: Windows::new(window, surface),
        gl_context: pcc,
        canvas,
    }
    .run(el)
}

impl<T: View + 'static> Element for T {
    fn insert(mut self, context: &mut impl InsertContext) {
        self.register(context.registry());

        app::iter_fields(self.as_reflect_mut(), |_, field| {
            if let Some(reflect_state) = context
                .registry()
                .get_type_data::<ReflectStateTrait>(field.type_id())
            {
                let Some(state) = reflect_state.get_mut(field) else {
                    return;
                };

                state.init()
            }
        });

        let built = self.build();

        let boxed = ViewWidget(Box::new(self)).into();

        let id = context.insert(boxed);
        context.child_work(built, id);

        // mount_children(registry, tree, id, built, idx)
    }

    fn compare_rebuild(
        mut self,
        old: MountedWidget,
        context: &mut impl RebuildContext,
    ) -> CompareResult<impl Element> {
        let MountedWidget::View(mut view) = old else {
            return CompareResult::Replace { with: self };
        };

        if self.type_id() != view.0.type_id() {
            return CompareResult::Replace { with: self };
        }

        app::iter_fields(self.as_reflect_mut(), |index, field| {
            if let Some(reflect_state) = context
                .registry()
                .get_type_data::<ReflectStateTrait>(field.type_id())
            {
                // todo uggly
                if let Some(state) = reflect_state.get_mut(field) {
                    if let bevy_reflect::ReflectMut::Struct(st) = view.0.reflect_mut() {
                        state.reuse(st.field_at_mut(index).unwrap());
                    } else if let bevy_reflect::ReflectMut::Enum(_) = view.0.reflect_mut() {
                        panic!();
                        // state.reuse(en.field_at_mut(index).unwrap());
                    } else {
                        panic!()
                    }
                }
            }
        });

        let built = self.build();

        // can be optimized
        *view.0.as_any_mut().downcast_mut::<Self>().unwrap() = self;

        context.insert(MountedWidget::View(view));

        context.child_work(built);

        return CompareResult::Success;
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum ClientMessage {
    SetState(String),
    RequestState,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Frame {
    data: Vec<RGBA8>,
    height: usize,
    width: usize,
    stride: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum ServerMessage {
    State(String),
    NoState,
}

/// Mostly a hack around functions being monomorphized at the call-site.
/// See [Element::insert_perform_per_child]
pub trait InsertContext {
    fn child_work<E: Element>(&mut self, e: E, parent: NodeId);
    fn insert(&mut self, m: MountedWidget) -> NodeId;
    fn registry(&mut self) -> &mut TypeRegistry;
}

/// Mostly a hack around functions being monomorphized at the call-site.
/// See [Element::compare_rebuild]
pub trait RebuildContext {
    fn child_work<E: Element>(&mut self, e: E);
    fn insert(&mut self, m: MountedWidget) -> NodeId;
    fn registry(&mut self) -> &mut TypeRegistry;
}

/// The result of a rebuild.
/// See [Element::compare_rebuild]
pub enum CompareResult<E> {
    Success,
    Replace { with: E },
}

/// Elements are some type that can be used to build a widget tree by inserting a [MountedWidget] at some given position.
/// Elements must also contain their own children, and perform any work the framework demands of them via [InsertContext] and [RebuildContext].
/// In some ways Elements are the bridge between both [View]s and [Widget]s, as it will commonly be implemented by both.
/// Usually one won't manually implement this trait (though, you can.), instead prefer to create [View]s.
pub trait Element: 'static {
    /// Each element is responsible for inserting into the tree.
    /// The element is expected to insert some [MountedWidget] that it represents into the tree via [InsertContext::insert].
    /// Additionally, if the element has any children (or just other elements that should be inserted underneath it), call [InsertContext::child_work] once per child.
    fn insert(self, context: &mut impl InsertContext);

    /// When the element tree is rebuilt because of a dirty view, the tree must be diffed. This function is called for each new element (returned by [View::build]) down the tree from the dirty widget,
    /// and it is the responsibility of that element to:
    /// * Compare itself to old. If old is not of the same type or otherwise incompatible with self, return a [CompareResult::Replace], with self.
    /// * If old can be used to build a new MountedWidget, rebuild. Reuse any allocations or state that has accumulated in the old element.
    /// * Additionally, if the new element has any children, call [RebuildContext::child_work] once per child.
    /// * Then return [CompareResult::Success], indicating a successful rebuild and insertion.
    fn compare_rebuild(
        self,
        old: MountedWidget,
        context: &mut impl RebuildContext,
    ) -> CompareResult<impl Element>;
}

/// Views are the building blocks of an application. They can be used to compose other views, elements, and any mix of the two.
///
/// ```
/// # use paladin_view::prelude::*;
///
/// #[derive(Reflect)]
/// struct CounterState(u32);
///
/// impl Reducer<ButtonMessage> for CounterState {
///     fn reduce(&mut self, message: ButtonMessage) {
///         self.0 += 1;
///     }
/// }
///
/// #[view]
/// struct Counter {
///     state: State<ButtonMessage, CounterState>
/// }
///
/// impl View for Counter {
///     fn build(&self) -> impl Element {
///         Text::builder().text(format!("{}", self.state.0)).build()
///     }
/// }
///
/// ```
///
pub trait View: DynView {
    fn build(&self) -> impl Element
    where
        Self: Sized;
}

#[doc(hidden)]
pub trait DynView: Reflect {
    fn register(&self, registry: &mut TypeRegistry);
    fn dyn_cmp(&self, child_id: NodeId, tree: &mut app::ElementTree, registry: &mut TypeRegistry);
}

pub struct Canvas {
    pub(crate) inner: femtovg::Canvas<OpenGl>,
    pub(crate) text_cache: text::RenderCache,
}

impl Canvas {
    fn font_system(&mut self) -> &mut FontSystem {
        &mut self.text_cache.font_system
    }

    fn clear_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: crate::Color) {
        self.inner.clear_rect(x, y, width, height, color.into())
    }
}

#[reflect_trait]
pub(crate) trait StateTrait {
    fn is_dirty(&self) -> bool;
    fn init(&mut self);
    fn reuse(&mut self, other: &mut dyn Reflect);
    fn process(&mut self);
}

/// A state reducer. It is generic over its message and is mostly used by [State] to handle a message sent to a given view.
pub trait Reducer<M> {
    fn reduce(&mut self, message: M);
}

#[derive(Reflect, Debug, Clone)]
#[reflect(StateTrait)]
/// Some state for a view.
/// State is the only way to change a view and expect it to corrextly re-render.
/// Since we use reflection, state must be stored as a field on a struct implementing [View] for it to work as expected.
/// ```
/// # use paladin_view::prelude::*;
///
/// #[derive(Reflect)]
/// struct CounterState(u32);
///
/// impl Reducer<ButtonMessage> for CounterState {
///     fn reduce(&mut self, message: ButtonMessage) {
///         self.0 += 1;
///     }
/// }
///
/// #[view]
/// struct Counter {
///     state: State<ButtonMessage, CounterState>
/// }
///
/// impl View for Counter {
///     fn build(&self) -> impl Element {
///         Text::builder().text(format!("{}", self.state.0)).build()
///     }
/// }
///
/// ```
pub struct State<M: Clone + 'static, S: Reducer<M> + 'static> {
    // #[reflect(ignore)]
    state: Option<S>,
    #[reflect(ignore)]
    // TODO: Should also be optional. No need to allocated if we haven't initted state yet.
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

impl<M: Message, S: Reducer<M> + 'static> StateTrait for State<M, S> {
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

impl<M: Message, S: Reducer<M> + 'static> Deref for State<M, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        self.state.as_ref().unwrap()
    }
}

impl<M: Message, S: Reducer<M> + 'static> DerefMut for State<M, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.state.as_mut().unwrap()
    }
}

impl<M: Message, S: Default + Reducer<M> + 'static> Default for State<M, S> {
    fn default() -> Self {
        Self {
            inner: MessageInner::default(),
            state: None,
            create_state: Default::default,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MessageInner<M> {
    rx: crossbeam::channel::Receiver<M>,
    tx: crossbeam::channel::Sender<M>,
}

impl<M> Default for MessageInner<M> {
    fn default() -> Self {
        let (tx, rx) = crossbeam::channel::unbounded();
        Self { rx, tx }
    }
}

impl<M: Clone + 'static, S: Reducer<M>> State<M, S> {
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
/// The result of layout out a widget with its given [Style].
/// It is passed into [Widget::render] and [Widget::layout] and should be respected to avoid clipping issues.
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

/// An action that can be triggered. Most commonly a on-click handler.
pub struct Triggerable {
    f: Box<dyn Fn()>,
}

impl Triggerable {
    pub fn trigger(&self) {
        (self.f)()
    }
}

impl<F: Fn() + 'static> From<F> for Triggerable {
    fn from(value: F) -> Self {
        Triggerable { f: Box::new(value) }
    }
}

#[doc(hidden)]
pub enum GlobalEvent {
    Dirty { hint: NodeId },
}

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
