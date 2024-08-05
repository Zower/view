pub use button::*;
pub use stack::Stack;
pub use text::Text;

mod button {
    use std::fmt::Debug;

    use bon::builder;

    use crate::{ElementTrait, SendableMessage};

    pub struct Button(SendableMessage);

    impl Debug for Button {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_tuple("Button").finish()
        }
    }

    impl ElementTrait for Button {
        fn clicked(&mut self) {
            println!("Clicking");
            self.0.send()
        }

        fn children(&mut self) -> &mut [crate::Element] {
            &mut []
        }
    }

    #[builder]
    pub fn button(on_click: SendableMessage) -> Button {
        Button(on_click)
    }
}

mod text {
    use crate::{Element, ElementTrait};

    #[derive(Debug)]
    pub struct Text;

    impl ElementTrait for Text {
        fn children(&mut self) -> &mut [Element] {
            &mut []
        }
    }
}

mod stack {
    use crate::{ChildView, Element, ElementTrait};

    #[derive(Debug)]
    pub struct Stack {
        tuple: Vec<Element>,
    }

    impl ElementTrait for Stack {
        fn children(&mut self) -> &mut [Element] {
            &mut self.tuple
        }
    }

    impl Stack {
        pub fn new<F>(child: impl ChildView<F>) -> Self {
            Self {
                tuple: child.to_element_vec(),
            }
        }
    }
}
