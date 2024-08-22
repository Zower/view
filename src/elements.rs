use std::{
    fmt::Debug,
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
};

use bevy_reflect::{ReflectMut, TypeRegistry};
pub use button::*;
pub use stack::HStack;
pub use stack::*;
use taffy::{prelude::auto, NodeId};
pub use text::*;

use crate::{
    app::{iter_fields, mount_children},
    Canvas, Element, PerChildElementThingy, ReflectStateTrait, View,
};

#[enum_delegate::register]
pub trait MountedElementBehaviour {
    #[allow(unused_variables)]
    fn event(&mut self, event: ElementEvent) {}

    fn style(&self) -> Style {
        Style::default()
    }

    #[allow(unused_variables)]
    fn layout(&mut self, layout: crate::Layout, canvas: &mut Canvas) {}

    #[allow(unused_variables)]
    fn render(&self, layout: crate::Layout, canvas: &mut Canvas) {}
}

pub enum RebuildResult {
    Rebuilt,
    Replace,
}

pub struct ViewElement(pub(crate) Box<dyn View>);

impl MountedElementBehaviour for ViewElement {}

#[derive(Debug)]
#[enum_delegate::implement(MountedElementBehaviour)]
pub enum MountableElement {
    Button(Button),
    Text(Text),
    HStack(HStack),
    View(ViewElement),
}

#[derive(Debug)]
pub struct ElementAndChildren<El, Children, F> {
    pub(crate) el: El,
    pub(crate) children: Children,
    phantom: PhantomData<F>,
}

impl<
        F: 'static,
        El: Into<MountableElement> + Element + 'static,
        Children: ChildView<F> + 'static,
    > Element for ElementAndChildren<El, Children, F>
{
    fn insert_perform_per_child(self, mut context: impl PerChildElementThingy) {
        let id = context.insert(self.el.into());

        struct TheBuilder<Pc: PerChildElementThingy> {
            pc: Pc,
            id: NodeId,
        }

        impl<Pc: PerChildElementThingy> ChildViewFnBuilder for TheBuilder<Pc> {
            fn build<E: Element>(&mut self) -> impl FnMut(E) {
                |e| self.pc.dothething(e, self.id)
            }
        }

        self.children.call_each(TheBuilder { pc: context, id });
    }

    fn try_reuse(&mut self, old: MountableElement, registry: &TypeRegistry) -> RebuildResult
    where
        Self: Sized,
    {
        self.el.try_reuse(old, registry)

        // if mem::discriminant(&old) != mem::discriminant(&this) {
        //     dbg!("NOT EQUAL");
        //     RebuildResult::Replace
        // } else {
        // }

        // todo!()
    }
}

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

pub enum ElementEvent {
    Click(u32, u32),
}

mod button {
    use std::fmt::Debug;

    use bevy_reflect::TypeRegistry;
    use bon::builder;

    use crate::{ButtonMessage, Color, Element, Layout, Reducer, State, Triggerable};

    use super::{ElementEvent, MountableElement, MountedElementBehaviour, RebuildResult};

    #[builder]
    pub struct Button {
        on_click: Triggerable,
    }

    impl Element for Button {
        // fn insert_perform_per_child(self) {
        // }

        fn try_reuse(&mut self, old: MountableElement, registry: &TypeRegistry) -> RebuildResult
        where
            Self: Sized,
        {
            // que?
            RebuildResult::Rebuilt
        }

        fn insert_perform_per_child(self, mut per_child: impl crate::PerChildElementThingy) {
            per_child.insert(MountableElement::Button(self));
        }
    }

    impl Button {
        pub fn on_click(on_click: Triggerable) -> Button {
            Self::builder().on_click(on_click).build()
        }

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

    impl MountedElementBehaviour for Button {
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
    use bevy_reflect::TypeRegistry;
    use bon::bon;
    use cosmic_text::{Attrs, AttrsList, Buffer, BufferLine, Metrics};

    use crate::{Element, PerChildElementThingy};

    use super::{MountedElementBehaviour, RebuildResult};

    #[derive(Debug)]
    pub struct Text {
        unused_text: Option<Vec<(String, AttrsList)>>,
        wrap: cosmic_text::Wrap,
        buffer: cosmic_text::Buffer,
    }

    impl Element for Text {
        fn insert_perform_per_child(self, mut context: impl PerChildElementThingy) {
            context.insert(super::MountableElement::Text(self));
        }

        fn try_reuse(
            &mut self,
            old: super::MountableElement,
            registry: &TypeRegistry,
        ) -> RebuildResult
        where
            Self: Sized,
        {
            // que?
            RebuildResult::Rebuilt
        }
    }

    #[bon]
    impl Text {
        #[builder]
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
    }

    impl MountedElementBehaviour for Text {
        fn layout(&mut self, layout: crate::Layout, canvas: &mut crate::Canvas) {
            if self.wrap != self.buffer.wrap() {
                self.buffer.set_wrap(canvas.font_system(), self.wrap);
            }

            let mut buffer = self.buffer.borrow_with(canvas.font_system());

            buffer.set_size(layout.size.width as f32, layout.size.height as f32);

            if let Some(text) = self.unused_text.take() {
                buffer.lines.clear();

                for (text, attrs) in text {
                    buffer.lines.push(BufferLine::new(
                        text,
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
    use std::marker::PhantomData;

    use bevy_reflect::TypeRegistry;

    use crate::Element;

    use super::{ChildView, ElementAndChildren, MountedElementBehaviour, RebuildResult};

    #[derive(Debug)]
    pub struct HStack;

    impl Element for HStack {
        fn insert_perform_per_child(self, mut context: impl crate::PerChildElementThingy) {
            context.insert(self.into());
        }

        fn try_reuse(
            &mut self,
            _old: super::MountableElement,
            _registry: &TypeRegistry,
        ) -> RebuildResult
        where
            Self: Sized,
        {
            RebuildResult::Rebuilt
        }
    }

    impl MountedElementBehaviour for HStack {
        fn style(&self) -> super::Style {
            super::Style::default().with_direction(taffy::FlexDirection::Row)
        }
    }

    pub fn hstack<F, CV: ChildView<F>>(child: CV) -> ElementAndChildren<HStack, CV, F> {
        ElementAndChildren {
            el: HStack,
            children: child,
            phantom: PhantomData,
        }
    }
}

pub enum OneOf<A, B> {
    A(A),
    B(B),
}

impl<A: Element, B: Element> Element for OneOf<A, B> {
    // fn (
    //     self,
    //     registry: &mut TypeRegistry,
    //     tree: &mut crate::app::ElementTree,
    //     parent: taffy::NodeId,
    //     idx: Option<usize>,
    // ) {
    // }

    fn try_reuse(&mut self, old: MountableElement, registry: &TypeRegistry) -> RebuildResult
    where
        Self: Sized,
    {
        match self {
            OneOf::A(a) => a.try_reuse(old, registry),
            OneOf::B(b) => b.try_reuse(old, registry),
        }
    }

    fn insert_perform_per_child(self, context: impl crate::PerChildElementThingy) {
        match self {
            OneOf::A(a) => a.insert_perform_per_child(context),
            OneOf::B(b) => b.insert_perform_per_child(context),
        }
    }
}

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

pub trait ChildViewFnBuilder {
    fn build<E: Element>(&mut self) -> impl FnMut(E);
}

pub trait ChildView<F> {
    fn call_each(self, f: impl ChildViewFnBuilder);
}

impl<A: Element> ChildView<(A,)> for A {
    fn call_each(self, mut f: impl ChildViewFnBuilder) {
        f.build()(self)
    }
}

impl<A: Element> ChildView<(A,)> for (A,) {
    fn call_each(self, mut f: impl ChildViewFnBuilder) {
        f.build()(self.0)
    }
}

impl<A: Element, B: Element> ChildView<(A, B)> for (A, B) {
    fn call_each(self, mut f: impl ChildViewFnBuilder) {
        f.build()(self.0);
        f.build()(self.1)
    }
}

impl<A: Element, B: Element, C: Element> ChildView<(A, B, C)> for (A, B, C) {
    fn call_each(self, mut f: impl ChildViewFnBuilder) {
        f.build()(self.0);
        f.build()(self.1);
        f.build()(self.2)
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

impl Debug for ViewElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ViewElement")
            .field(&self.0.as_reflect())
            .finish()
    }
}
