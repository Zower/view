use bevy_reflect::{ParsedPath, Reflect};
use view::{hstack, run, view, Button, Element, Messages, ReflectView, Text, View};

fn main() -> view::Result<()> {
    run(MyView {
        // second: MySecondView::default(),
        second: AnotherView::False(Messages::default(), MySecondView::default()),
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
    second: AnotherView,
    // second: MySecondView,
    messages: Messages<MySecondViewMessage>,
}

impl View for MyView {
    fn build(&self) -> Element {
        view!(&self.second).into()
    }
}

#[derive(Reflect)]
#[reflect(View)]
enum AnotherView {
    False(Messages<MySecondViewMessage>, MySecondView),
    True(PlusOne, Messages<MySecondViewMessage>),
}

impl View for AnotherView {
    fn build(&self) -> Element {
        let messages = match self {
            AnotherView::False(m, _) => m,
            AnotherView::True(_, m) => m,
        };

        hstack((
            match self {
                AnotherView::False(poo, _) => ParsedPath::parse_static(".1").unwrap(),
                AnotherView::True(poo, _) => ParsedPath::parse_static(".0").unwrap(),
            },
            Button::on_click(messages.send(MySecondViewMessage::Clicked)),
        ))
    }

    fn messages(&mut self) {
        let messages = match self {
            AnotherView::False(m, _) => m,
            AnotherView::True(_, m) => m,
        };

        let mut to_modify = false;

        while let Some(_) = messages.recv() {
            to_modify = true;
        }

        if !to_modify {
            return;
        }

        *self = match self {
            AnotherView::False(messages, _) => AnotherView::True(PlusOne(0), messages.clone()),
            AnotherView::True(_, messages) => {
                AnotherView::False(messages.clone(), MySecondView::default())
            }
        }
    }
}
#[derive(Reflect, Default, Debug)]
#[reflect(View)]
struct MySecondView {
    messages: Messages<MySecondViewMessage>,
    view: PlusOne,
    data: u64,
}

impl View for MySecondView {
    fn build(&self) -> Element {
        hstack((
            Text::builder().text("Hey from second!").build(),
            Button::on_click(self.messages.send(MySecondViewMessage::Clicked)),
            view!(&self.view),
        ))
    }

    fn messages(&mut self) {
        while let Some(message) = self.messages.recv() {
            match message {
                MySecondViewMessage::Clicked => {
                    self.data += 1;
                    self.view.0 = self.data + 1;
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
