use bevy_reflect::Reflect;

#[derive(Clone, Copy, Reflect)]
pub enum ButtonMessage {
    Clicked(u32, u32),
}
