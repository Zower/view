use bevy_reflect::{GetTypeRegistration, Reflect};
use view::{hstack, run, Button, ButtonMessage, Element, IntoElement, Receiver, State, Text, View};
use view_macros::{view, Register};

fn main() -> view::Result<()> {
    run(Root)
}

#[view]
struct Root;

impl View for Root {
    fn build(&self) -> impl Element {
        MyView {
            state: State::create_state(|| MyViewState::False),
        }
    }
}

#[view]
struct MyView {
    state: State<ButtonMessage, MyViewState>,
}

impl View for MyView {
    fn build(&self) -> impl Element {
        hstack((
            match *self.state {
                MyViewState::False => MySecondView::default().element(),
                MyViewState::True(data) => PlusOne(data).element(),
            },
            Button::interactions(&self.state),
        ))
    }
}

#[derive(Reflect, Debug, Clone)]
enum MyViewState {
    False,
    True(u32),
}

impl Receiver for MyViewState {
    type Message = ButtonMessage;

    fn reduce(&mut self, message: Self::Message) {
        match message {
            ButtonMessage::Clicked(_, _) => {
                *self = match self {
                    MyViewState::False => MyViewState::True(0),
                    MyViewState::True(_) => MyViewState::False,
                };
            }
        }
    }
}

#[derive(Reflect, Default)]
struct MySecondViewState(u32);

impl Receiver for MySecondViewState {
    type Message = ButtonMessage;

    fn reduce(&mut self, message: Self::Message) {
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
    fn build(&self) -> impl Element {
        hstack((
            // Text::builder().text("Hey from second!").build(),
            Text::builder().text(format!("{}", self.state.0)).build(),
            Button::interactions(&self.state),
            PlusOne(self.state.0 + 1),
        ))
    }
}

#[view]
struct PlusOne(u32);

impl View for PlusOne {
    fn build(&self) -> impl Element {
        hstack((
            Text::builder().text(format!("{}", self.0)).build(),
            Text::builder().text(format!("{}", self.0)).build(),
            Text::builder().text(format!("{}", self.0)).build(),
        ))
    }
}
