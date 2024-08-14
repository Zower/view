use bevy_reflect::Reflect;
use view::{hstack, run, Button, Element, IntoElement, Message, ReflectView, State, Text, View};

fn main() -> view::Result<()> {
    run(MyView {
        state: State::default(),
    })
}

#[derive(Reflect, Debug, Clone)]
enum MySecondViewMessage {
    Clicked,
}

impl Message for MySecondViewMessage {
    type State = u32;

    fn reduce(self, state: &mut Self::State) {
        match self {
            MySecondViewMessage::Clicked => *state += 1,
        }
    }
}

#[derive(Reflect)]
#[reflect(View)]
struct MyView {
    state: State<u32, MySecondViewMessage>,
}

impl View for MyView {
    fn build(&self) -> Element {
        AnotherView::True(State::default()).into()
    }
}

#[derive(Reflect)]
#[reflect(View)]
enum AnotherView {
    False(State<u32, MySecondViewMessage>),
    True(State<u32, MySecondViewMessage>),
}

#[derive(Reflect, Debug, Clone)]
enum AnotherViewMessage {
    Clicked,
}

impl Message for AnotherViewMessage {
    type State = u32;

    fn reduce(self, state: &mut Self::State) {
        match self {
            AnotherViewMessage::Clicked => *state += 1,
        }
    }
}

impl View for AnotherView {
    fn build(&self) -> Element {
        let messages = match self {
            AnotherView::False(m) => m,
            AnotherView::True(m) => m,
        };

        hstack((
            match self {
                AnotherView::False(state) => PlusOne(**state).element(),
                AnotherView::True(_) => MySecondView::default().element(),
            },
            Button::on_click(messages.then_send(MySecondViewMessage::Clicked)),
        ))
    }
}
#[derive(Reflect, Default, Debug)]
#[reflect(View)]
struct MySecondView {
    state: State<u32, MySecondViewMessage>,
}

impl View for MySecondView {
    fn build(&self) -> Element {
        hstack((
            // Text::builder().text("Hey from second!").build(),
            Text::builder().text(format!("{}", *self.state)).build(),
            Button::on_click(self.state.then_send(MySecondViewMessage::Clicked)),
            PlusOne(*self.state + 1),
        ))
    }
}

#[derive(Reflect, Default, Debug)]
#[reflect(View)]
struct PlusOne(u32);

impl View for PlusOne {
    fn build(&self) -> Element {
        hstack((
            Text::builder().text(format!("{}", self.0)).build(),
            Text::builder().text(format!("{}", self.0)).build(),
            Text::builder().text(format!("{}", self.0)).build(),
        ))
    }
}
