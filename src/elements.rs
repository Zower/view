use std::ops::{Deref, DerefMut};

pub use button::*;
pub use stack::Stack;
use taffy::prelude::auto;
pub use text::Text;

use crate::Canvas;

#[derive(Debug)]
#[enum_delegate::implement(ElementTrait)]
pub enum Element {
    Button(Button),
    Text(Text),
    Stack(Stack),
}

#[enum_delegate::register]
pub trait ElementTrait {
    #[allow(unused_variables)]
    fn event(&mut self, event: ElementEvent) {}

    fn style(&self) -> Style {
        Style::default()
    }

    #[allow(unused_variables)]
    fn render(&self, layout: crate::Layout, canvas: &mut Canvas) {}

    fn consume(self) -> (Element, Option<Vec<Element>>);
}

#[derive(Debug, Clone)]
pub struct Style(pub taffy::Style);

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

    use crate::{Color, Layout, SendableMessage};

    use super::{Element, ElementEvent, ElementTrait};

    pub struct Button(SendableMessage);

    impl Debug for Button {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_tuple("Button").finish()
        }
    }

    impl ElementTrait for Button {
        fn event(&mut self, event: ElementEvent) {
            match event {
                ElementEvent::Click(_, _) => self.0.send(),
            };
        }

        fn render(&self, layout: Layout, canvas: &mut crate::Canvas) {
            dbg!(&layout);
            canvas.clear_rect(
                layout.location.x,
                layout.location.y,
                layout.size.width,
                layout.size.height,
                Color::rgb(200, 130, 90),
            );
        }

        fn consume(self) -> (Element, Option<Vec<Element>>) {
            (self.into(), None)
        }
    }

    #[builder]
    pub fn button(on_click: SendableMessage) -> Button {
        Button(on_click)
    }
}

mod text {
    use super::{Element, ElementTrait};

    #[derive(Debug)]
    pub struct Text;

    impl ElementTrait for Text {
        fn consume(self) -> (Element, Option<Vec<Element>>) {
            (self.into(), None)
        }
    }
}

mod stack {

    use super::{ChildView, Element, ElementTrait};

    #[derive(Debug)]
    pub struct Stack {
        tuple: Option<Vec<Element>>,
    }

    impl ElementTrait for Stack {
        fn consume(mut self) -> (Element, Option<Vec<Element>>) {
            let tuple = self.tuple.take();
            (self.into(), tuple)
        }
    }

    impl Stack {
        pub fn new<F>(child: impl ChildView<F>) -> Self {
            Self {
                tuple: Some(child.to_element_vec()),
            }
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
impl<A: Into<Element>, B: Into<Element>> ChildView<(A, B)> for (A, B) {
    fn to_element_vec(self) -> Vec<Element> {
        vec![self.0.into(), self.1.into()]
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
