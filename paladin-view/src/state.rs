use std::ops::{Deref, DerefMut};

use bevy_reflect::{reflect_trait, Reflect};
use crossbeam::channel::TryRecvError;

use crate::Triggerable;

#[reflect_trait]
pub(crate) trait StateTrait {
    fn is_dirty(&self) -> bool;
    fn init(&mut self);
    fn reuse(&mut self, other: &mut dyn Reflect);
    fn process(&mut self);
}

/// A state reducer. It is generic over its message and is mostly used by [State] to handle a message sent to a given view.
pub trait Reducer<M> {
    fn reduce(&mut self, message: M);
}

#[derive(Reflect, Debug, Clone)]
#[reflect(StateTrait)]
/// Some state for a view.
/// State is the only way to change a view and expect it to correctly re-render.
/// Since we use reflection, state must be stored as a field on a struct implementing [View] for it to work as expected.
/// ```
/// # use paladin_view::prelude::*;
///
/// #[derive(Reflect)]
/// struct CounterState(u32);
///
/// impl Reducer<ButtonMessage> for CounterState {
///     fn reduce(&mut self, message: ButtonMessage) {
///         self.0 += 1;
///     }
/// }
///
/// #[view]
/// struct Counter {
///     state: State<ButtonMessage, CounterState>
/// }
///
/// impl View for Counter {
///     fn build(&self) -> impl Element {
///         Text::builder().text(format!("{}", self.state.0)).build()
///     }
/// }
///
/// ```
pub struct State<M: Clone + 'static, S: Reducer<M> + 'static> {
    // #[reflect(ignore)]
    state: Option<S>,
    #[reflect(ignore)]
    // TODO: Should also be optional. No need to allocated if we haven't initted state yet.
    inner: MessageInner<M>,
    #[reflect(ignore)]
    #[reflect(default = "create_state_fake")]
    create_state: fn() -> S,
}

impl Reducer<()> for () {
    fn reduce(&mut self, _: ()) {}
}

pub(crate) trait Message: Clone + 'static {}

impl<T: Clone + 'static> Message for T {}

fn create_state_fake<S>() -> fn() -> S {
    panic!()
}

impl<M: Message, S: Reducer<M> + 'static> StateTrait for State<M, S> {
    fn is_dirty(&self) -> bool {
        !self.inner.rx.is_empty()
    }

    fn process(&mut self) {
        while let Some(message) = self.recv() {
            self.deref_mut().reduce(message);
        }
    }

    fn init(&mut self) {
        self.state = Some((self.create_state)());
    }

    fn reuse(&mut self, other: &mut dyn Reflect) {
        let selfy = other.as_any_mut().downcast_mut::<Self>().unwrap();

        std::mem::swap(&mut self.state, &mut selfy.state);
    }
}

impl<M: Message, S: Reducer<M> + 'static> Deref for State<M, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        self.state.as_ref().unwrap()
    }
}

impl<M: Message, S: Reducer<M> + 'static> DerefMut for State<M, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.state.as_mut().unwrap()
    }
}

impl<M: Message, S: Default + Reducer<M> + 'static> Default for State<M, S> {
    fn default() -> Self {
        Self {
            inner: MessageInner::default(),
            state: None,
            create_state: Default::default,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MessageInner<M> {
    rx: crossbeam::channel::Receiver<M>,
    tx: crossbeam::channel::Sender<M>,
}

impl<M> Default for MessageInner<M> {
    fn default() -> Self {
        let (tx, rx) = crossbeam::channel::unbounded();
        Self { rx, tx }
    }
}

impl<M: Clone + 'static, S: Reducer<M>> State<M, S> {
    pub fn create_state(f: fn() -> S) -> Self {
        Self {
            inner: MessageInner::default(),
            state: None,
            create_state: f,
        }
    }

    pub fn then_send(&self, message: M) -> Triggerable {
        let sender = self.inner.tx.clone();
        Triggerable {
            f: Box::new(move || {
                if let Err(err) = sender.send(message.clone()) {
                    dbg!("WARN: ", err);
                }
            }),
        }
    }

    fn recv(&self) -> Option<M> {
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
}
