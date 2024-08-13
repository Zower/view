use bevy_reflect::Reflect;
use view::{hstack, run, Button, Element, IntoElement, Messages, ReflectView, Text, View};

fn main() -> view::Result<()> {
    run(MyView {
        messages: Messages::default(),
    })
}

#[derive(Reflect, Debug, Clone)]
enum MySecondViewMessage {
    Clicked,
}

#[derive(Reflect)]
#[reflect(View)]
struct MyView {
    // second: MySecondView,
    messages: Messages<MySecondViewMessage>,
}

impl View for MyView {
    fn build(&self) -> Element {
        AnotherView::True(Messages::default()).into()
    }
}

#[derive(Reflect)]
#[reflect(View)]
enum AnotherView {
    False(Messages<MySecondViewMessage>, u32),
    True(Messages<MySecondViewMessage>),
}

impl View for AnotherView {
    fn build(&self) -> Element {
        let messages = match self {
            AnotherView::False(m, _) => m,
            AnotherView::True(m) => m,
        };

        hstack((
            match self {
                AnotherView::False(_, data) => PlusOne(*data as u64).element(),
                AnotherView::True(_) => MySecondView::default().element(),
            },
            Button::on_click(messages.send(MySecondViewMessage::Clicked)),
        ))
    }

    fn messages(&mut self) {
        let (messages, data) = match self {
            AnotherView::False(m, data) => (m, data),
            AnotherView::True(m) => (m, &mut 0),
        };

        let mut to_modify = false;

        while let Some(msg) = messages.recv() {
            *data += 1;
            to_modify = true;
        }

        if !to_modify {
            return;
        }

        return;

        *self = match self {
            AnotherView::False(messages, _) => AnotherView::True(messages.clone()),
            AnotherView::True(messages) => AnotherView::False(messages.clone(), 0),
        }
    }
}
#[derive(Reflect, Default, Debug)]
#[reflect(View)]
struct MySecondView {
    messages: Messages<MySecondViewMessage>,
    data: u64,
}

impl View for MySecondView {
    fn build(&self) -> Element {
        hstack((
            Text::builder().text("Hey from second!").build(),
            Button::on_click(self.messages.send(MySecondViewMessage::Clicked)),
            PlusOne(self.data + 1),
        ))
    }

    fn messages(&mut self) {
        while let Some(message) = self.messages.recv() {
            match message {
                MySecondViewMessage::Clicked => {
                    self.data += 1;
                }
            }
        }
    }
}

#[derive(Reflect, Default, Debug)]
#[reflect(View)]
struct PlusOne(u64);

impl View for PlusOne {
    fn build(&self) -> Element {
        hstack((
            Text::builder().text(format!("{}", self.0)).build(),
            Text::builder().text(format!("{}", self.0)).build(),
            Text::builder().text(format!("{}", self.0)).build(),
        ))
    }
}
