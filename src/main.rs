use bevy_reflect::Reflect;
use view::{hstack, run, view, Button, Element, Messages, ReflectView, Text, View};

fn main() -> view::Result<()> {
    run(MyView {
        second: MySecondView::default(),
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
    second: MySecondView,
    messages: Messages<MySecondViewMessage>,
}

impl View for MyView {
    fn build(&self) -> Element {
        view!(&self.second).into()
    }

    fn messages(&mut self) {
        return;
        while let Some(message) = self.messages.recv() {
            match message {
                MySecondViewMessage::Clicked => {
                    self.second.data += 1;
                    self.second.view.0 += 1;
                }
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
        return;
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
