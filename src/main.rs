use bevy_reflect::Reflect;
use view::{button, run, Element, Messages, ReflectView, Stack, Text, View};

fn main() -> view::Result<()> {
    run(MyView {
        second: MySecondView::default(),
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
}

impl View for MyView {
    fn build(&self) -> Element {
        Stack::new((Text, &self.second)).into()
    }
}

#[derive(Reflect, Default, Debug)]
#[reflect(View)]
struct MySecondView {
    messages: Messages<MySecondViewMessage>,
    view: Third,
    data: u64,
}

impl View for MySecondView {
    fn build(&self) -> Element {
        Stack::new((
            button()
                .on_click(self.messages.send(MySecondViewMessage::Clicked))
                .call(),
            &self.view,
        ))
        .into()
    }

    fn messages(&mut self) {
        while let Some(message) = self.messages.recv() {
            dbg!(message);
        }
    }
}

#[derive(Reflect, Default, Debug)]
#[reflect(View)]
struct Third;

impl View for Third {
    fn build(&self) -> Element {
        Text.into()
    }
}
