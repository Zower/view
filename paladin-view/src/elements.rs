pub use button::*;
use cosmic_text::FontSystem;
pub use stack::HStack;
pub use stack::*;
use std::{
    any::Any,
    fmt::Debug,
    ops::{Deref, DerefMut},
};
use taffy::{prelude::auto, NodeId};
pub use text::*;

use crate::{Canvas, CompareResult, Element, InsertContext, Layout, RebuildContext, View};

/// An element that has been mounted into the tree.
#[derive(Debug)]
#[enum_delegate::implement(Widget)]
pub enum MountedWidget {
    Button(Button),
    Text(Text),
    HStack(HStack),
    View(ViewWidget),
    Custom(CustomWidget),
}

pub struct CustomWidget(pub Box<dyn AnyWidget>);

pub trait AnyWidget: Any {
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    fn render(&self, layout: crate::Layout, canvas: &mut Canvas);
}

impl<T: Any + Widget> AnyWidget for T {
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn render(&self, layout: crate::Layout, canvas: &mut Canvas) {
        self.render(layout, canvas)
    }
}

impl Widget for CustomWidget {
    fn event(&mut self, event: ElementEvent) {
        self.0.event(event)
    }

    fn style(&self) -> Style {
        self.0.style()
    }

    fn layout(&mut self, layout: Layout, font_system: &mut FontSystem) {
        self.0.layout(layout, font_system)
    }

    fn render(&self, layout: crate::Layout, canvas: &mut Canvas) {
        self.0.render(layout, canvas)
    }
}

#[enum_delegate::register]
/// The behavior of an element that is inserted into the tree.
pub trait Widget {
    /// Respond to events that may occur while this Widget is mouunted.
    /// This is where state updates happen.
    ///
    /// ```
    /// use paladin_view::prelude::*;
    ///
    /// // A button that stores a dynamic function that could send a message, update a mutex, or similar.
    /// struct Button(Box<dyn Fn()>);
    ///
    /// // Imagine we are inserted into the tree..
    ///
    /// impl Widget for Button {
    ///     fn event(&mut self, event: ElementEvent) {
    ///         if matches!(event, ElementEvent::Click(_, _)) {
    ///             (self.0)()
    ///         }
    ///     }
    /// }
    ///
    /// ```
    #[allow(unused_variables)]
    fn event(&mut self, event: ElementEvent) {}

    /// Return the current style of the element. This may be called up to each frame.
    fn style(&self) -> Style {
        Style::default()
    }

    #[allow(unused_variables)]
    /// A function where a [Widget] can perform layout calculations within its given bounds. This is most useful to layout text paragraphs before rendering.
    /// Most widgets only paint based on some immutable data and do not need to implement this function.
    ///
    /// ```
    /// # use paladin_view::prelude::*;
    ///
    /// struct Text(cosmic_text::Buffer);
    ///
    /// // Imagine we are inserted into the tree..
    ///
    /// impl Widget for Text {
    ///     fn layout(&mut self, layout: Layout, canvas: &mut Canvas) {
    ///         let mut buffer = self.0.borrow_with(canvas.font_system());
    ///         buffer.set_size(layout.size.width as f32, layout.size.height as f32);
    ///         buffer.shape_until_scroll(true);
    ///     }
    ///
    ///     fn render(&self, layout: Layout, canvas: &mut Canvas) {
    ///         // ..
    ///     }
    /// }
    ///
    /// ```
    fn layout(&mut self, layout: Layout, font_system: &mut cosmic_text::FontSystem) {}

    /// Painting.
    /// ```
    /// # use paladin_view::prelude::*;
    ///
    /// struct FixedRect;
    ///
    /// // Imagine we are inserted into the tree..
    ///
    /// impl Widget for FixedRect {
    ///     fn render(&self, layout: Layout, canvas: &mut impl Canvas) {
    ///         canvas.clear_rect(
    ///             layout.location.x,
    ///             layout.location.y,
    ///             100,
    ///             100,
    ///             Color::rgb(200, 130, 90).into(),
    ///         );
    ///     }
    /// }
    ///
    /// ```
    #[allow(unused_variables)]
    fn render(&self, layout: crate::Layout, canvas: &mut Canvas) {}
}

/// A [View] that has been mounted into the tree as a [MountedWidget::View].
pub struct ViewWidget(pub(crate) Box<dyn View>);

impl Widget for ViewWidget {}

/// The style of a widget. Styling decides final layout (size, position) and is based on the flexbox algorithm, thanks to [taffy].
#[derive(Debug, Clone)]
pub struct Style(pub taffy::Style);

impl Style {
    pub fn with_direction(mut self, direction: taffy::FlexDirection) -> Self {
        self.0.flex_direction = direction;

        self
    }
}

impl Default for Style {
    fn default() -> Self {
        Self(taffy::Style {
            size: taffy::Size {
                width: taffy::Dimension::Percent(1.),
                height: auto(),
            },
            ..Default::default()
        })
    }
}

/// Any interaction with an element.
pub enum ElementEvent {
    Click(u32, u32),
}

mod button {
    use std::fmt::Debug;

    use bon::builder;

    use crate::{ButtonMessage, Color, Element, Layout, Reducer, State, Triggerable};

    use super::{ElementEvent, MountedWidget, Widget};

    #[builder]
    pub struct Button {
        on_click: Triggerable,
    }

    impl Element for Button {
        fn insert(self, context: &mut impl crate::InsertContext) {
            context.insert(MountedWidget::Button(self));
        }

        fn compare_rebuild(
            self,
            old: MountedWidget,
            context: &mut impl crate::RebuildContext,
        ) -> crate::CompareResult<impl Element> {
            if matches!(old, MountedWidget::Button(_)) {
                context.insert(MountedWidget::Button(self));
                crate::CompareResult::<Self>::Success
            } else {
                crate::CompareResult::Replace { with: self }
            }
        }
    }

    impl Button {
        /// A button that performs some action when clicked.
        ///
        /// See also [State].
        /// ```
        /// # use paladin_view::prelude::*;
        /// #[view]
        /// struct Printer;
        ///
        /// impl View for Printer {
        ///     fn build(&self) -> impl Element {
        ///         Button::on_click(|| println!("Hello world!"))
        ///     }
        /// }
        ///
        /// ```
        ///
        pub fn on_click(on_click: impl Into<Triggerable>) -> Button {
            Self::builder().on_click(on_click).build()
        }

        /// Convenience for a state reducer that only responds to button messages.
        pub fn interactions<S: Reducer<ButtonMessage>>(state: &State<ButtonMessage, S>) -> Button {
            Self::builder()
                .on_click(state.then_send(ButtonMessage::Clicked(0, 0)))
                .build()
        }
    }

    impl Debug for Button {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_tuple("Button").finish()
        }
    }

    impl PartialEq for Button {
        fn eq(&self, _other: &Self) -> bool {
            false
        }
    }

    impl Widget for Button {
        fn event(&mut self, event: ElementEvent) {
            match event {
                ElementEvent::Click(_, _) => self.on_click.trigger(),
            };
        }

        fn render(&self, layout: Layout, canvas: &mut crate::Canvas) {
            canvas.clear_rect(
                layout.location.x,
                layout.location.y,
                layout.size.width,
                layout.size.height,
                Color::rgb(200, 130, 90).into(),
            );
        }
    }
}

mod text {
    use bon::bon;
    use cosmic_text::{Attrs, AttrsList, Buffer, BufferLine, FontSystem, LineEnding, Metrics};

    use crate::{Element, InsertContext, RebuildContext};

    use super::{MountedWidget, Widget};

    #[derive(Debug)]
    /// Rich text.
    pub struct Text {
        unused_text: Option<Vec<(String, AttrsList)>>,
        wrap: cosmic_text::Wrap,
        buffer: cosmic_text::Buffer,
    }

    impl Element for Text {
        fn insert(self, context: &mut impl InsertContext) {
            context.insert(super::MountedWidget::Text(self));
        }

        fn compare_rebuild(
            self,
            old: super::MountedWidget,
            context: &mut impl RebuildContext,
        ) -> crate::CompareResult<impl Element> {
            if matches!(old, MountedWidget::Text(_)) {
                context.insert(MountedWidget::Text(self));
                crate::CompareResult::<Self>::Success
            } else {
                crate::CompareResult::Replace { with: self }
            }
        }
    }

    #[bon]
    impl Text {
        #[builder]
        /// Create a text widget.
        /// Like all widgets, uses the builder syntax from [bon].
        /// ```
        ///
        /// # use paladin_view::prelude::*;
        ///
        /// Text::builder().text("Hello!").size(28.).build();
        ///
        /// ```
        ///
        pub fn new(
            text: impl Into<String>,
            color: Option<crate::Color>,
            wrap: Option<cosmic_text::Wrap>,
            font: Option<&'static str>,
            size: Option<f32>,
        ) -> impl Element {
            let size = size.unwrap_or(25.);
            let attrs = Attrs::new()
                .color(color.unwrap_or_default().into())
                .family(cosmic_text::Family::Name(font.unwrap_or("JetBrains Mono")));

            Self {
                unused_text: Some(vec![(text.into(), AttrsList::new(attrs))]),
                buffer: Buffer::new_empty(Metrics::new(size, size)),
                wrap: wrap.unwrap_or(cosmic_text::Wrap::Word),
            }
        }

        #[builder]
        pub fn rich(text: Vec<(String, AttrsList)>, size: f32) -> Text {
            Self {
                unused_text: Some(text),
                wrap: cosmic_text::Wrap::Word,
                buffer: Buffer::new_empty(Metrics::new(size, size)),
            }
        }
    }

    impl Text {}

    fn text(str: &'static str) -> Text {
        let size = 25.;
        let attrs = Attrs::new()
            .color(crate::Color::default().into())
            .family(cosmic_text::Family::Name("JetBrains Mono"));

        Text {
            unused_text: Some(vec![(str.into(), AttrsList::new(attrs))]),
            buffer: Buffer::new_empty(Metrics::new(size, size)),
            wrap: cosmic_text::Wrap::Word,
        }
    }

    impl Element for &'static str {
        fn insert(self, context: &mut impl InsertContext) {
            context.insert(super::MountedWidget::Text(text(self)));
        }

        fn compare_rebuild(
            self,
            old: MountedWidget,
            context: &mut impl RebuildContext,
        ) -> crate::CompareResult<impl Element> {
            if matches!(old, MountedWidget::Text(_)) {
                context.insert(MountedWidget::Text(text(self)));
                crate::CompareResult::<Self>::Success
            } else {
                crate::CompareResult::Replace { with: self }
            }
        }
    }

    impl Widget for Text {
        fn layout(&mut self, layout: crate::Layout, font_system: &mut FontSystem) {
            if self.wrap != self.buffer.wrap() {
                self.buffer.set_wrap(font_system, self.wrap);
            }

            let mut buffer = self.buffer.borrow_with(font_system);

            buffer.set_size(
                Some(layout.size.width as f32),
                Some(layout.size.height as f32),
            );

            if let Some(text) = self.unused_text.take() {
                buffer.lines.clear();

                for (text, attrs) in text {
                    buffer.lines.push(BufferLine::new(
                        text,
                        LineEnding::default(),
                        attrs,
                        // This _MUST_ be advanced for coloring to work.
                        // Otherwise the colors appear to apply per-word instead of per-byte? Not sure, but leave as is.
                        cosmic_text::Shaping::Advanced,
                    ));
                }
            }

            // if self.buffer_needs_refresh {
            buffer.shape_until_scroll(true);
            // }
        }

        fn render(&self, layout: crate::Layout, canvas: &mut crate::Canvas) {
            let text_draw_cmds = canvas
                .text_cache
                .fill_buffer_to_draw_commands(
                    &mut canvas.inner,
                    &self.buffer,
                    (layout.location.x as f32, layout.location.y as f32),
                )
                .unwrap();

            for (color, cmds) in text_draw_cmds {
                canvas.draw_glyph_commands(
                    cmds,
                    &femtovg::Paint::color(femtovg::Color::rgb(color.r(), color.g(), color.b())),
                    1.,
                );
            }
        }
    }
}

mod stack {

    use std::{fmt::Debug, marker::PhantomData};

    use crate::{CompareResult, Element};

    use super::{ChildInsertBuilder, ChildRebuildBuilder, ChildView, MountedWidget, Widget};

    #[derive(Debug)]
    pub struct HStack;

    pub(crate) struct HStackElement<F, Children: ChildView<F>> {
        children: Children,
        phantom: PhantomData<F>,
    }

    impl<F, Children: ChildView<F>> Element for HStackElement<F, Children>
    where
        F: 'static,
        Children: 'static,
    {
        fn insert(self, context: &mut impl crate::InsertContext) {
            let id = context.insert(super::MountedWidget::HStack(HStack));

            self.children
                .call_each(ChildInsertBuilder { pc: context, id })
        }

        fn compare_rebuild(
            self,
            old: super::MountedWidget,
            context: &mut impl crate::RebuildContext,
        ) -> CompareResult<impl Element + 'static> {
            if !matches!(old, MountedWidget::HStack(_)) {
                return CompareResult::Replace { with: self };
            }

            context.insert(super::MountedWidget::HStack(HStack));

            self.children.call_each(ChildRebuildBuilder { pc: context });

            crate::CompareResult::<Self>::Success
        }
    }

    impl Widget for HStack {
        fn style(&self) -> super::Style {
            super::Style::default().with_direction(taffy::FlexDirection::Row)
        }
    }

    #[allow(private_bounds)]
    /// A horizontal stack, also called a Row.
    ///
    /// ```
    /// # use paladin_view::prelude::*;
    ///
    /// hstack(
    ///     (
    ///         "Hello",
    ///         "World !"
    ///     )
    /// );
    ///
    /// ```
    pub fn hstack<F: 'static, CV: ChildView<F> + 'static>(child: CV) -> impl Element {
        HStackElement {
            children: child,
            phantom: PhantomData,
        }
    }
}

pub(crate) mod prelude {
    pub use super::button::Button;
    pub use super::stack::{hstack, HStack};
    pub use super::text::Text;
    pub use super::OneOf;
    pub use super::OneOfSwizz;
}

/// Allows returning different types from a expression, assuming they both implement [Element].
///
/// This won't compile:
///
/// ```compile_fail
///
/// # use paladin_view::prelude::*;
/// # let some_condition = true;
///
/// if some_condition {
///     "Doesnt compile"
/// } else {
///     Button::on_click(|| {})
/// }
///
/// ```
/// Instead, use OneOf with its convenience function [OneOfSwizz::left] and [OneOfSwizz::right]:
///
/// ```
/// # use paladin_view::prelude::*;
/// # let some_condition = true;
///
/// let _ = if some_condition {
///     "Compiles :)".left()
/// } else {
///     Button::on_click(|| {}).right()
/// };
///
/// ```
#[derive(Debug)]
pub enum OneOf<A, B> {
    A(A),
    B(B),
}

impl<A: Element, B: Element> Element for OneOf<A, B> {
    fn insert(self, context: &mut impl crate::InsertContext) {
        match self {
            OneOf::A(a) => a.insert(context),
            OneOf::B(b) => b.insert(context),
        }
    }

    fn compare_rebuild(
        self,
        old: MountedWidget,
        context: &mut impl RebuildContext,
    ) -> CompareResult<impl Element> {
        match self {
            OneOf::A(a) => match a.compare_rebuild(old, context) {
                CompareResult::Success => CompareResult::Success,
                CompareResult::Replace { with } => CompareResult::Replace {
                    with: OneOf::<_, _>::A(with),
                },
            },
            OneOf::B(b) => match b.compare_rebuild(old, context) {
                CompareResult::Success => CompareResult::Success,
                CompareResult::Replace { with } => CompareResult::Replace {
                    with: OneOf::<_, _>::B(with),
                },
            },
        }
    }
}

/// Convenience methods for generating [OneOf]
pub trait OneOfSwizz<A> {
    fn left<B>(self) -> OneOf<A, B>;
    fn right<B>(self) -> OneOf<B, A>;
}

impl<El> OneOfSwizz<El> for El {
    fn left<B>(self) -> OneOf<El, B> {
        OneOf::A(self)
    }

    fn right<B>(self) -> OneOf<B, El> {
        OneOf::B(self)
    }
}

pub(crate) trait ChildViewFnBuilder {
    fn create_fn<E: Element>(&mut self) -> impl FnMut(E);
}

struct ChildInsertBuilder<'a, Pc: InsertContext> {
    pc: &'a mut Pc,
    id: NodeId,
}

struct ChildRebuildBuilder<'a, Pc: RebuildContext> {
    pc: &'a mut Pc,
}

impl<'a, Pc: InsertContext> ChildViewFnBuilder for ChildInsertBuilder<'a, Pc> {
    fn create_fn<E: Element>(&mut self) -> impl FnMut(E) {
        |e| self.pc.child_work(e, self.id)
    }
}

impl<'a, Pc: RebuildContext> ChildViewFnBuilder for ChildRebuildBuilder<'a, Pc> {
    fn create_fn<E: Element>(&mut self) -> impl FnMut(E) {
        |e| self.pc.child_work(e)
    }
}

pub(crate) trait ChildView<F> {
    fn call_each(self, f: impl ChildViewFnBuilder);
}

impl<A: Element> ChildView<(A,)> for A {
    fn call_each(self, mut f: impl ChildViewFnBuilder) {
        f.create_fn()(self)
    }
}

impl<A: Element> ChildView<(A,)> for (A,) {
    fn call_each(self, mut f: impl ChildViewFnBuilder) {
        f.create_fn()(self.0)
    }
}

impl<A: Element, B: Element> ChildView<(A, B)> for (A, B) {
    fn call_each(self, mut f: impl ChildViewFnBuilder) {
        f.create_fn()(self.0);
        f.create_fn()(self.1)
    }
}

impl<A: Element, B: Element, C: Element> ChildView<(A, B, C)> for (A, B, C) {
    fn call_each(self, mut f: impl ChildViewFnBuilder) {
        f.create_fn()(self.0);
        f.create_fn()(self.1);
        f.create_fn()(self.2)
    }
}

impl Deref for Style {
    type Target = taffy::Style;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Style {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl std::fmt::Debug for ViewWidget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ViewElement")
            .field(&self.0.as_reflect())
            .finish()
    }
}

impl std::fmt::Debug for CustomWidget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CustomWidget").finish()
    }
}
