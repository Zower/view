use bevy_reflect::{reflect_trait, GetPath, GetTypeRegistration, Reflect, TypeRegistry};

mod elements;
pub mod patch;

use crossbeam::channel::TryRecvError;
pub use elements::*;

pub fn run<V: View + GetTypeRegistration + GetPath>(mut v: V) {
    let mut type_registry = TypeRegistry::new();

    type_registry.register::<V>();

    let mut built = v.build();

    iter_elements(&mut built, &|el| {
        let el = dbg!(el);
        el.clicked();
    });

    iter_views(&type_registry, &mut v, &|item| {
        item.messages();
    });
}

#[reflect_trait]
pub trait View: Reflect {
    fn build(&self) -> Element;
    fn messages(&mut self) {}
}

impl<V: View> From<&V> for Element {
    fn from(value: &V) -> Self {
        value.build()
    }
}

#[enum_delegate::register]
pub trait ElementTrait {
    fn clicked(&mut self) {}
    fn children(&mut self) -> &mut [Element];
}

#[derive(Debug)]
#[enum_delegate::implement(ElementTrait)]
pub enum Element {
    Button(elements::Button),
    Text(elements::Text),
    Stack(elements::Stack),
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

#[derive(Reflect, Debug)]
pub struct Messages<M> {
    #[reflect(ignore)]
    inner: Inner<M>,
}

impl<M> Default for Messages<M> {
    fn default() -> Self {
        Self {
            inner: Inner::default(),
        }
    }
}

#[derive(Debug)]
pub struct Inner<M> {
    rx: crossbeam::channel::Receiver<M>,
    tx: crossbeam::channel::Sender<M>,
}

impl<M> Default for Inner<M> {
    fn default() -> Self {
        let (tx, rx) = crossbeam::channel::unbounded();
        Self { rx, tx }
    }
}

impl<M: Clone + 'static> Messages<M> {
    pub fn send(&self, message: M) -> SendableMessage {
        let sender = self.inner.tx.clone();
        SendableMessage {
            f: Box::new(move || {
                sender.send(message.clone()).expect("Failed to send");
            }),
        }
    }

    pub fn recv(&self) -> Option<M> {
        self.inner
            .rx
            .try_recv()
            .inspect_err(|f| {
                let TryRecvError::Empty = f else {
                    panic!("Closed channel")
                };
            })
            .ok()
    }

    // pub fn send(&self, m: M) -> bool {
    //     let Ok(mut data) = self.data.lock() else {
    //         return false;
    //     };

    //     data.push(m);

    //     true
    // }
}

pub struct SendableMessage {
    f: Box<dyn Fn()>,
}

impl SendableMessage {
    pub fn send(&self) {
        (self.f)()
    }
}

fn iter_views(reg: &TypeRegistry, item: &mut dyn Reflect, f: &impl Fn(&mut dyn View)) {
    let t = reg.get_type_data::<ReflectView>(item.type_id());

    match t {
        Some(it) => {
            let v: &mut dyn View = it.get_mut(item).unwrap();
            f(v)
        }
        None => {}
    }

    match item.reflect_mut() {
        bevy_reflect::ReflectMut::Struct(s) => {
            // iter(reg, s.iter_fields(), f)
            let mut index = 0;

            while let Some(item) = s.field_at_mut(index) {
                index += 1;

                iter_views(reg, item, f)
            }
        }
        // bevy_reflect::ReflectRef::Struct(s) => {
        //     for item in s.iter_fields() {
        //         iter_views(reg, item, f)
        //     }
        // }
        // bevy_reflect::ReflectRef::TupleStruct(_) => todo!(),
        // bevy_reflect::ReflectRef::Tuple(_) => todo!(),
        // bevy_reflect::ReflectRef::List(_) => todo!(),
        // bevy_reflect::ReflectRef::Array(_) => todo!(),
        // bevy_reflect::ReflectRef::Map(_) => todo!(),
        bevy_reflect::ReflectMut::Enum(e) => {
            // iter(reg, e.iter_fields().map(|it| it.value()), f)
            let mut index = 0;

            while let Some(item) = e.field_at_mut(index) {
                index += 1;

                iter_views(reg, item, f)
            }
        }
        bevy_reflect::ReflectMut::Value(_) => {}
        _ => todo!(), // bevy_reflect::ReflectRef::Value(_) => todo!(),
    }
}

fn iter_elements(item: &mut Element, f: &impl Fn(&mut Element)) {
    f(item);

    for item in item.children() {
        iter_elements(item, f)
    }
}
