use bevy_reflect::TypeRegistry;
pub use button::*;
use cosmic_text::FontSystem;
pub use stack::HStack;
pub use stack::*;
use std::{
    any::Any,
    fmt::Debug,
    ops::{Deref, DerefMut},
};
use taffy::{prelude::auto, LengthPercentage};
pub use text::*;

use crate::{
    BuildResult, Canvas, Element, InsertChildren, InsertContext, KeyEvent, Layout, RebuildChildren,
    RebuildContext,
};

/// An element that has been mounted into the tree.
#[derive(Debug)]
#[enum_delegate::implement(Widget)]
pub enum MountedWidget {
    Button(Button),
    Text(Text),
    HStack(HStack),
    Custom(CustomWidget),
}

pub struct CustomWidget(pub Box<dyn AnyWidget>);

pub trait AnyWidget: Any {
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    fn render(&self, layout: crate::Layout, canvas: &mut Canvas);
    fn event(&mut self, event: WidgetEvent);
    fn layout(&mut self, layout: Layout, font_system: &mut FontSystem);
    fn style(&self) -> Style;
}

impl<T: Any + Widget> AnyWidget for T {
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn render(&self, layout: crate::Layout, canvas: &mut Canvas) {
        self.render(layout, canvas)
    }

    fn event(&mut self, event: WidgetEvent) {
        self.event(event);
    }

    fn layout(&mut self, layout: Layout, font_system: &mut FontSystem) {
        self.layout(layout, font_system);
    }

    fn style(&self) -> Style {
        self.style()
    }
}

impl Widget for CustomWidget {
    fn event(&mut self, event: WidgetEvent) {
        self.0.event(event)
    }

    fn style(&self) -> Style {
        self.0.style()
    }

    fn layout(&mut self, layout: Layout, font_system: &mut FontSystem) {
        self.0.layout(layout, font_system)
    }

    fn render(&self, layout: Layout, canvas: &mut Canvas) {
        self.0.render(layout, canvas)
    }
}

#[enum_delegate::register]
/// An element (or whatever it has decided to insert) that is inserted into the tree.
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
    fn event(&mut self, event: WidgetEvent) {}

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
pub enum WidgetEvent {
    Click(u32, u32),
    Key(KeyEvent),
}

/// Shorthands for styling.
pub trait Styleable: Sized {
    fn style_mut(&mut self) -> &mut Style;

    fn pad(mut self, padding: LengthPercentage) -> Self {
        self.style_mut().0.padding = taffy::Rect {
            left: padding,
            right: padding,
            top: padding,
            bottom: padding,
        };

        self
    }

    // fn align(mut self, align: ) -> Self {
    //     self.style_mut().0.ali

    //     self
    // }
}

mod button {
    use std::fmt::Debug;

    use bevy_reflect::TypeRegistry;
    use bon::builder;

    use crate::{
        state::{Reducer, State},
        ButtonMessage, Color, Element, Layout, LeafNode, Triggerable,
    };

    use super::{MountedWidget, Style, Styleable, Widget, WidgetEvent};

    #[builder]
    pub struct Button {
        on_click: Triggerable,
        style: Style,
    }

    impl Element for Button {
        #[allow(refining_impl_trait)]
        fn create(self, _: &mut TypeRegistry) -> crate::BuildResult<LeafNode> {
            crate::BuildResult {
                widget: MountedWidget::Button(self),
                children: None,
            }
        }

        #[allow(refining_impl_trait)]
        fn compare_rebuild(self, _: MountedWidget) -> crate::BuildResult<LeafNode> {
            crate::BuildResult {
                widget: MountedWidget::Button(self),
                children: None,
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
            Self::builder()
                .on_click(on_click)
                .style(Style::default())
                .build()
        }

        /// Convenience for a state reducer that only responds to button messages.
        pub fn interactions<S: Reducer<ButtonMessage>>(state: &State<ButtonMessage, S>) -> Button {
            Self::builder()
                .on_click(state.then_send(ButtonMessage::Clicked(0, 0)))
                .style(Style::default())
                .build()
        }
    }

    impl Widget for Button {
        fn event(&mut self, event: WidgetEvent) {
            if let WidgetEvent::Click(_, _) = event {
                self.on_click.trigger()
            };
        }

        fn style(&self) -> Style {
            self.style.clone()
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

    impl Styleable for Button {
        fn style_mut(&mut self) -> &mut Style {
            &mut self.style
        }
    }

    impl Debug for Button {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_tuple("Button").finish()
        }
    }
}

mod text {
    use bevy_reflect::TypeRegistry;
    use bon::bon;
    use cosmic_text::{Attrs, AttrsList, Buffer, BufferLine, FontSystem, LineEnding, Metrics};

    use crate::{Element, LeafNode};

    use super::{MountedWidget, Style, Styleable, Widget};

    #[derive(Debug)]
    /// Rich text.
    pub struct Text {
        unused_text: Option<Vec<(String, AttrsList)>>,
        wrap: cosmic_text::Wrap,
        buffer: cosmic_text::Buffer,
        style: Style,
    }

    impl Element for Text {
        #[allow(refining_impl_trait)]
        fn create(self, _: &mut TypeRegistry) -> crate::BuildResult<LeafNode> {
            crate::BuildResult {
                widget: MountedWidget::Text(self),
                children: None,
            }
        }

        #[allow(refining_impl_trait)]
        fn compare_rebuild(self, _: MountedWidget) -> crate::BuildResult<LeafNode> {
            // todo
            crate::BuildResult {
                widget: MountedWidget::Text(self),
                children: None,
            }
        }

        // fn compare_rebuild(
        //     self,
        //     old: super::MountedWidget,
        //     context: &mut impl RebuildContext,
        // ) -> crate::CompareResult<impl Element> {
        //     if matches!(old, MountedWidget::Text(_)) {
        //         // todo
        //         context.insert(MountedWidget::Text(self));
        //         crate::CompareResult::<Self>::Success
        //     } else {
        //         crate::CompareResult::Replace { with: self }
        //     }
        // }
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
        ) -> Text {
            let size = size.unwrap_or(25.);
            let attrs = Attrs::new()
                .color(color.unwrap_or_default().into())
                .family(cosmic_text::Family::Name(font.unwrap_or("JetBrains Mono")));

            Self {
                unused_text: Some(vec![(text.into(), AttrsList::new(attrs))]),
                buffer: Buffer::new_empty(Metrics::new(size, size)),
                wrap: wrap.unwrap_or(cosmic_text::Wrap::Word),
                style: Style::default(),
            }
        }

        #[builder]
        pub fn rich(text: Vec<(String, AttrsList)>, size: f32) -> Text {
            Self {
                unused_text: Some(text),
                wrap: cosmic_text::Wrap::Word,
                buffer: Buffer::new_empty(Metrics::new(size, size)),
                style: Style::default(),
            }
        }
    }

    fn text(str: &'static str) -> Text {
        let size = 25.;
        let attrs = Attrs::new()
            .color(crate::Color::default().into())
            .family(cosmic_text::Family::Name("JetBrains Mono"));

        Text {
            unused_text: Some(vec![(str.into(), AttrsList::new(attrs))]),
            buffer: Buffer::new_empty(Metrics::new(size, size)),
            wrap: cosmic_text::Wrap::Word,
            style: Style::default(),
        }
    }

    impl Element for &'static str {
        #[allow(refining_impl_trait)]
        fn create(self, _: &mut TypeRegistry) -> crate::BuildResult<LeafNode> {
            crate::BuildResult {
                widget: MountedWidget::Text(text(self)),
                children: None,
            }
        }

        #[allow(refining_impl_trait)]
        fn compare_rebuild(self, _: MountedWidget) -> crate::BuildResult<LeafNode> {
            crate::BuildResult {
                widget: MountedWidget::Text(text(self)),
                children: None,
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
                canvas.inner.draw_glyph_commands(
                    cmds,
                    &femtovg::Paint::color(femtovg::Color::rgb(color.r(), color.g(), color.b())),
                    1.,
                );
            }
        }

        fn style(&self) -> Style {
            self.style.clone()
        }
    }

    impl Styleable for Text {
        fn style_mut(&mut self) -> &mut Style {
            &mut self.style
        }
    }
}

mod stack {

    use std::{fmt::Debug, marker::PhantomData};

    use bevy_reflect::TypeRegistry;

    use crate::{BuildResult, Element, InsertChildren, RebuildChildren};

    use super::{ChildInsertBuilder, ChildRebuildBuilder, ChildView, Widget};

    #[derive(Debug)]
    pub struct HStack;

    pub struct HStackElement<F, Children: ChildView<F>> {
        children: Children,
        phantom: PhantomData<F>,
    }

    pub(crate) struct HStackChildren<F, Children: ChildView<F>> {
        children: Children,
        phantom: PhantomData<F>,
    }

    impl<F: 'static, C: ChildView<F> + 'static> RebuildChildren for HStackChildren<F, C> {
        fn rebuild_children(self, builder: &mut impl crate::RebuildContext) {
            self.children.call_each(ChildRebuildBuilder { pc: builder });
        }
    }

    impl<F: 'static, C: ChildView<F> + 'static> InsertChildren for HStackChildren<F, C> {
        fn insert_children(self, builder: &mut impl crate::InsertContext) {
            self.children.call_each(ChildInsertBuilder { pc: builder });
        }
    }

    impl<F, Children: ChildView<F>> Element for HStackElement<F, Children>
    where
        F: 'static,
        Children: 'static,
    {
        fn create(self, _: &mut TypeRegistry) -> BuildResult<impl InsertChildren> {
            crate::BuildResult {
                widget: super::MountedWidget::HStack(HStack),
                children: Some(HStackChildren {
                    children: self.children,
                    phantom: PhantomData,
                }),
            }
        }

        fn compare_rebuild(self, _: super::MountedWidget) -> BuildResult<impl RebuildChildren> {
            // if !matches!(old, MountedWidget::HStack(_)) {
            //     return CompareResult::Replace { with: self };
            // }

            // context.insert(super::MountedWidget::HStack(HStack));

            // self.children.call_each(ChildRebuildBuilder { pc: context });
            crate::BuildResult {
                widget: super::MountedWidget::HStack(HStack),
                children: Some(HStackChildren {
                    children: self.children,
                    phantom: PhantomData,
                }),
            }

            // crate::CompareResult::<Self>::Success
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
    #[allow(private_interfaces)]
    pub fn hstack<F: 'static, CV: ChildView<F> + 'static>(child: CV) -> HStackElement<F, CV> {
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
    pub use super::Styleable;
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
    fn create(self, registry: &mut TypeRegistry) -> crate::BuildResult<impl InsertChildren> {
        match self {
            OneOf::A(a) => {
                let result = a.create(registry);
                BuildResult {
                    widget: result.widget,
                    children: result.children.map(|children| OneOf::<_, _>::A(children)),
                }
            }
            OneOf::B(b) => {
                let result = b.create(registry);

                BuildResult {
                    widget: result.widget,
                    children: result.children.map(|children| OneOf::<_, _>::B(children)),
                }
            }
        }
    }

    fn compare_rebuild(self, old: MountedWidget) -> BuildResult<impl RebuildChildren> {
        match self {
            OneOf::A(a) => {
                let result = a.compare_rebuild(old);
                BuildResult {
                    widget: result.widget,
                    children: result.children.map(|children| OneOf::<_, _>::A(children)),
                }
            }
            OneOf::B(b) => {
                let result = b.compare_rebuild(old);

                BuildResult {
                    widget: result.widget,
                    children: result.children.map(|children| OneOf::<_, _>::B(children)),
                }
            }
        }
    }
}

impl<A: RebuildChildren, B: RebuildChildren> RebuildChildren for OneOf<A, B> {
    fn rebuild_children(self, context: &mut impl RebuildContext) {
        match self {
            OneOf::A(a) => a.rebuild_children(context),
            OneOf::B(b) => b.rebuild_children(context),
        }
    }
}

impl<A: InsertChildren, B: InsertChildren> InsertChildren for OneOf<A, B> {
    fn insert_children(self, context: &mut impl InsertContext) {
        match self {
            OneOf::A(a) => a.insert_children(context),
            OneOf::B(b) => b.insert_children(context),
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
}

struct ChildRebuildBuilder<'a, Pc: RebuildContext> {
    pc: &'a mut Pc,
}

impl<'a, Pc: InsertContext> ChildViewFnBuilder for ChildInsertBuilder<'a, Pc> {
    fn create_fn<E: Element>(&mut self) -> impl FnMut(E) {
        |e| self.pc.insert_child(e)
    }
}

impl<'a, Pc: RebuildContext> ChildViewFnBuilder for ChildRebuildBuilder<'a, Pc> {
    fn create_fn<E: Element>(&mut self) -> impl FnMut(E) {
        |e| self.pc.rebuild_child(e)
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

impl std::fmt::Debug for CustomWidget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CustomWidget").finish()
    }
}
