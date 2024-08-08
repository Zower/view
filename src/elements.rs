use std::ops::{Deref, DerefMut};

use bevy_reflect::{Access, ParsedPath};
pub use button::*;
pub use stack::HStack;
pub use stack::*;
use taffy::prelude::auto;
pub use text::*;

use crate::Canvas;

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

    fn needs_rebuild(&self, new: &Self) -> bool;
}

#[derive(Debug)]
#[enum_delegate::implement(MountedElementBehaviour)]
pub(crate) enum MountableElement {
    Button(Button),
    Text(Text),
    HStack(HStack),
}

impl MountableElement {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            MountableElement::Button(_) => "button",
            MountableElement::Text(_) => "text",
            MountableElement::HStack(_) => "hstack",
        }
    }
}

#[derive(Debug)]
pub enum ElementOrPath {
    Element(Element),
    Path(ParsedPath),
}

impl From<ParsedPath> for ElementOrPath {
    fn from(value: ParsedPath) -> Self {
        ElementOrPath::Path(value)
    }
}

impl From<ParsedPath> for Element {
    fn from(value: ParsedPath) -> Self {
        Element {
            el: HStack.into(),
            children: Some(vec![ElementOrPath::Path(value)]),
        }
    }
}

#[derive(Debug)]
pub struct Element {
    pub(crate) el: MountableElement,
    pub(crate) children: Option<Vec<ElementOrPath>>,
}

impl Element {
    pub(crate) fn no_children(el: MountableElement) -> Self {
        Self { el, children: None }
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

    use bon::builder;

    use crate::{Color, Layout, Triggerable};

    use super::{Element, ElementEvent, ElementOrPath, MountedElementBehaviour};

    #[builder]
    pub struct Button {
        on_click: Triggerable,
    }

    impl Button {
        pub fn on_click(on_click: Triggerable) -> Button {
            Self::builder().on_click(on_click).build()
        }
    }

    impl From<Button> for ElementOrPath {
        fn from(value: Button) -> Self {
            ElementOrPath::Element(Element::no_children(value.into()))
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

        fn needs_rebuild(&self, new: &Self) -> bool {
            true
        }
    }

    // pub fn button(on_click: Triggerable) -> Button {
    // Button(on_click)
    // }
}

mod text {
    use bon::bon;
    use cosmic_text::{Attrs, AttrsList, Buffer, BufferLine, Metrics};

    use super::{Element, ElementOrPath, MountedElementBehaviour};

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

    impl From<Text> for ElementOrPath {
        fn from(value: Text) -> Self {
            ElementOrPath::Element(Element::no_children(value.into()))
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

        fn needs_rebuild(&self, new: &Self) -> bool {
            let Some(unused_text) = &new.unused_text else {
                panic!("Rebuild with built text??")
            };

            // Todo needs more
            !self
                .buffer
                .lines
                .iter()
                .map(|it| it.text())
                .inspect(|it| {
                    dbg!(&it);
                })
                .eq(unused_text.iter().map(|it| &it.0).inspect(|it| {
                    dbg!(&it);
                }))
        }
    }
}

mod stack {
    use super::{ChildView, Element, MountedElementBehaviour};

    #[derive(Debug)]
    pub struct HStack;

    impl MountedElementBehaviour for HStack {
        fn style(&self) -> super::Style {
            super::Style::default().with_direction(taffy::FlexDirection::Row)
        }

        fn needs_rebuild(&self, new: &Self) -> bool {
            // actually this needs to check children? what does this mean?
            false
        }
    }

    pub fn hstack<F>(child: impl ChildView<F>) -> Element {
        Element {
            el: HStack.into(),
            children: Some(child.to_element_vec()),
        }
    }
}

impl From<Element> for ElementOrPath {
    fn from(value: Element) -> Self {
        ElementOrPath::Element(value)
    }
}

pub trait ChildView<F> {
    fn to_element_vec(self) -> Vec<ElementOrPath>;
}

impl<A: Into<ElementOrPath>> ChildView<(A,)> for A {
    fn to_element_vec(self) -> Vec<ElementOrPath> {
        vec![self.into()]
    }
}

impl<A: Into<ElementOrPath>, B: Into<ElementOrPath>> ChildView<(A, B)> for (A, B) {
    fn to_element_vec(self) -> Vec<ElementOrPath> {
        vec![self.0.into(), self.1.into()]
    }
}

impl<A: Into<ElementOrPath>, B: Into<ElementOrPath>, C: Into<ElementOrPath>> ChildView<(A, B, C)>
    for (A, B, C)
{
    fn to_element_vec(self) -> Vec<ElementOrPath> {
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
