use core::panic;
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
    thread::panicking,
};

use bevy_reflect::{ReflectMut, TypeRegistry};
pub use button::*;
pub use stack::HStack;
pub use stack::*;
use taffy::prelude::auto;
pub use text::*;

use crate::{app::iter_fields, Canvas, ReflectStateTrait, View};

#[enum_delegate::register]
pub(crate) trait MountedElementBehaviour {
    #[allow(unused_variables)]
    fn event(&mut self, event: ElementEvent) {}

    fn style(&self) -> Style {
        Style::default()
    }

    #[allow(unused_variables)]
    fn layout(&mut self, layout: crate::Layout, canvas: &mut Canvas) {}

    #[allow(unused_variables)]
    fn render(&self, layout: crate::Layout, canvas: &mut Canvas) {}

    fn try_reuse(&mut self, old: Self, registry: &TypeRegistry) -> RebuildResult
    where
        Self: Sized;
}

pub(crate) enum RebuildResult {
    Rebuilt,
    Replace,
}

pub struct ViewElement(pub(crate) Box<dyn View>);

impl MountedElementBehaviour for ViewElement {
    fn try_reuse(&mut self, mut old: Self, registry: &TypeRegistry) -> RebuildResult
    where
        Self: Sized,
    {
        if self.0.type_id() != old.0.type_id() {
            return RebuildResult::Replace;
        }

        iter_fields(self.0.as_reflect_mut(), |index, field| {
            if let Some(reflect_state) =
                registry.get_type_data::<ReflectStateTrait>(field.type_id())
            {
                if let Some(state) = reflect_state.get_mut(field) {
                    if let ReflectMut::Struct(st) = old.0.reflect_mut() {
                        state.reuse(st.field_at_mut(index).unwrap());
                    } else if let ReflectMut::Enum(en) = old.0.reflect_mut() {
                        panic!();
                        // state.reuse(en.field_at_mut(index).unwrap());
                    } else {
                        panic!()
                    }
                }
            }
        });

        RebuildResult::Rebuilt
    }
}

#[derive(Debug)]
#[enum_delegate::implement(MountedElementBehaviour)]
pub(crate) enum MountableElement {
    Button(Button),
    Text(Text),
    HStack(HStack),
    View(ViewElement),
}

#[derive(Debug)]
pub struct Element {
    pub(crate) el: MountableElement,
    pub(crate) children: Option<Vec<Element>>,
}

impl Element {
    pub(crate) fn no_children(el: MountableElement) -> Self {
        Self { el, children: None }
    }
}

impl<V: View> From<V> for Element {
    fn from(value: V) -> Self {
        Element {
            el: MountableElement::View(ViewElement(Box::new(value))),
            children: None,
        }
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

    use crate::{Color, Layout, Triggerable};

    use super::{Element, ElementEvent, MountedElementBehaviour, RebuildResult};

    #[builder]
    pub struct Button {
        on_click: Triggerable,
    }

    impl Button {
        pub fn on_click(on_click: Triggerable) -> Button {
            Self::builder().on_click(on_click).build()
        }
    }

    impl From<Button> for Element {
        fn from(value: Button) -> Self {
            Element::no_children(value.into())
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

        fn try_reuse(&mut self, _: Self, _: &TypeRegistry) -> RebuildResult
        where
            Self: Sized,
        {
            RebuildResult::Rebuilt
        }
    }

    // pub fn button(on_click: Triggerable) -> Button {
    // Button(on_click)
    // }
}

mod text {
    use bevy_reflect::TypeRegistry;
    use bon::bon;
    use cosmic_text::{Attrs, AttrsList, Buffer, BufferLine, Metrics};

    use super::{Element, MountedElementBehaviour, RebuildResult};

    #[derive(Debug)]
    pub struct Text {
        unused_text: Option<Vec<(String, AttrsList)>>,
        wrap: cosmic_text::Wrap,
        buffer: cosmic_text::Buffer,
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
        ) -> Element {
            let size = size.unwrap_or(25.);
            let attrs = Attrs::new()
                .color(color.unwrap_or_default().into())
                .family(cosmic_text::Family::Name(font.unwrap_or("JetBrains Mono")));

            Element {
                el: super::MountableElement::Text(Self {
                    unused_text: Some(vec![(text.into(), AttrsList::new(attrs))]),
                    buffer: Buffer::new_empty(Metrics::new(size, size)),
                    wrap: wrap.unwrap_or(cosmic_text::Wrap::Word),
                }),
                children: None,
            }
        }
    }

    impl From<Text> for Element {
        fn from(value: Text) -> Self {
            Element::no_children(value.into())
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

        fn try_reuse(&mut self, _: Self, _: &TypeRegistry) -> RebuildResult
        where
            Self: Sized,
        {
            RebuildResult::Rebuilt
        }
    }
}

mod stack {
    use bevy_reflect::TypeRegistry;

    use super::{ChildView, Element, MountedElementBehaviour, RebuildResult};

    #[derive(Debug)]
    pub struct HStack;

    impl MountedElementBehaviour for HStack {
        fn style(&self) -> super::Style {
            super::Style::default().with_direction(taffy::FlexDirection::Row)
        }

        fn try_reuse(&mut self, _: Self, _: &TypeRegistry) -> RebuildResult
        where
            Self: Sized,
        {
            RebuildResult::Rebuilt
        }
    }

    pub fn hstack<F>(child: impl ChildView<F>) -> Element {
        Element {
            el: HStack.into(),
            children: Some(child.to_element_vec()),
        }
    }
}

pub trait ChildView<F> {
    fn to_element_vec(self) -> Vec<Element>;
}

impl<A: Into<Element>> ChildView<(A,)> for A {
    fn to_element_vec(self) -> Vec<Element> {
        vec![self.into()]
    }
}

impl<A: Into<Element>> ChildView<(A,)> for (A,) {
    fn to_element_vec(self) -> Vec<Element> {
        vec![self.0.into()]
    }
}

impl<A: Into<Element>, B: Into<Element>> ChildView<(A, B)> for (A, B) {
    fn to_element_vec(self) -> Vec<Element> {
        vec![self.0.into(), self.1.into()]
    }
}

impl<A: Into<Element>, B: Into<Element>, C: Into<Element>> ChildView<(A, B, C)> for (A, B, C) {
    fn to_element_vec(self) -> Vec<Element> {
        vec![self.0.into(), self.1.into(), self.2.into()]
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
