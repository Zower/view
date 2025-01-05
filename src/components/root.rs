use paladin_view::{prelude::*, taffy::LengthPercentage};

use crate::BufferElement;

#[view]
pub struct Root;

impl View for Root {
    fn build(&self) -> impl Element + use<> {
        MyView {
            state: State::create_state(|| MyViewState { data: 0 }),
        }
    }
}

#[view]
struct MyView {
    state: State<ButtonMessage, MyViewState>,
}

impl View for MyView {
    fn build(&self) -> impl Element + use<> {
        // "Some beautiful text"
        hstack((
            BufferElement::new("src/main.rs").pad(LengthPercentage::Percent(0.5)),
            MySecondView::default(),
        ))
    }
}

#[derive(Reflect, Debug, Clone)]
struct MyViewState {
    data: u32,
}

impl Reducer<ButtonMessage> for MyViewState {
    fn reduce(&mut self, message: ButtonMessage) {
        match message {
            ButtonMessage::Clicked(_, _) => {
                self.data += 1;
            }
        }
    }
}

#[derive(Reflect, Default, Debug)]
struct MySecondViewState(u32);

impl Reducer<ButtonMessage> for MySecondViewState {
    fn reduce(&mut self, message: ButtonMessage) {
        match message {
            ButtonMessage::Clicked(_, _) => self.0 += 1,
        }
    }
}

#[view]
#[derive(Default)]
struct MySecondView {
    state: State<ButtonMessage, MySecondViewState>,
}

impl View for MySecondView {
    fn build(&self) -> impl Element + use<> {
        hstack((
            Text::builder().text(format!("{}", self.state.0)).build(),
            Button::interactions(&self.state),
            PlusOne(self.state.0 + 1),
        ))
    }
}

#[view]
struct PlusOne(u32);

impl View for PlusOne {
    fn build(&self) -> impl Element + use<> {
        hstack((
            Text::builder().text(format!("{}", self.0)).build(),
            Text::builder().text(format!("{}", self.0)).build(),
            Text::builder().text(format!("{}", self.0)).build(),
        ))
    }
}
