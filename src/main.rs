use bevy_reflect::{GetTypeRegistration, Reflect, TypeRegistry};
use view::{hstack, run, Button, Element, IntoElement, Message, ReflectView, State, Text, View};

fn main() -> view::Result<()> {
    run(Root)
}

#[derive(Reflect)]
#[reflect(View)]
struct Root;

impl View for Root {
    fn build(&self) -> Element {
        MyView {
            state: State::create_state(|| MyViewState::False),
        }
        .into()
    }

    fn register(&self, registry: &mut TypeRegistry) {
        registry.register::<Self>();
        Self::register_type_dependencies(registry);
    }
}

#[derive(Reflect)]
#[reflect(View)]
struct MyView {
    state: State<MyViewState, MyViewMessage>,
}

impl View for MyView {
    fn build(&self) -> Element {
        hstack((
            match *self.state {
                MyViewState::False => MySecondView::default().element(),
                MyViewState::True(data) => PlusOne(data).element(),
            },
            Button::on_click(self.state.then_send(MyViewMessage::Change)),
        ))
    }

    fn register(&self, registry: &mut TypeRegistry) {
        registry.register::<Self>();
        Self::register_type_dependencies(registry);
    }
}

#[derive(Reflect, Debug, Clone)]
enum MyViewState {
    False,
    True(u32),
}

#[derive(Reflect, Debug, Clone)]
enum MyViewMessage {
    Change,
}

impl Message for MyViewMessage {
    type State = MyViewState;

    fn reduce(self, state: &mut Self::State) {
        match self {
            MyViewMessage::Change => {
                *state = match state {
                    MyViewState::False => MyViewState::True(0),
                    MyViewState::True(_) => MyViewState::False,
                }
            }
        }
    }
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

    fn register(&self, registry: &mut TypeRegistry) {
        registry.register::<Self>();
        Self::register_type_dependencies(registry);
    }
}

#[derive(Reflect, Default, Debug)]
#[reflect(View)]
struct PlusOne(u32);

impl View for PlusOne {
    fn build(&self) -> Element {
        dbg!("---------------------------------------");
        hstack((
            Text::builder().text(format!("{}", self.0)).build(),
            Text::builder().text(format!("{}", self.0)).build(),
            Text::builder().text(format!("{}", self.0)).build(),
        ))
    }

    fn register(&self, registry: &mut TypeRegistry) {
        registry.register::<Self>();
        Self::register_type_dependencies(registry);
    }
}
