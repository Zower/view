use std::collections::{HashMap, VecDeque};

use bevy_reflect::{GetPath, GetTypeRegistration, Reflect, TypeRegistry};
use taffy::{prelude::length, NodeId, Size, TaffyTree};
use winit::dpi::PhysicalSize;

use crate::{
    Canvas, Element, Layout, MountableElement, MountedElementBehaviour, Point, ReflectState,
    ReflectView, View,
};

pub struct App<V> {
    taffy: TaffyTree,
    registry: TypeRegistry,
    view: V,
    elements: HashMap<NodeId, MountableElement>,
    root: NodeId,
}

pub enum AppEvent {
    Clicked(u32, u32),
}

impl<V: View + GetTypeRegistration + GetPath> App<V> {
    pub fn new(view: V, size: PhysicalSize<u32>) -> Self {
        let mut type_registry = TypeRegistry::new();

        type_registry.register::<V>();

        let mut taffy: TaffyTree = Default::default();

        let root = taffy
            .new_leaf(taffy::Style {
                size: taffy::Size {
                    width: length(size.width as f32),
                    height: length(size.height as f32),
                },
                ..Default::default()
            })
            .unwrap();

        let map = HashMap::default();

        let mut this = Self {
            taffy,
            registry: type_registry,
            view,
            root,
            elements: map,
        };

        mount_elements(
            &mut this.taffy,
            this.root,
            this.view.build(),
            &mut |node_id, item| {
                this.elements.insert(node_id, item);
            },
        );

        this
    }
}

impl<V: View> App<V> {
    pub fn event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Clicked(x, y) => {
                for (_, node) in Self::iter_elements(&self.taffy, self.root) {
                    let el = self.elements.get_mut(&node).unwrap();

                    el.event(crate::ElementEvent::Click(x, y));
                }
            }
        }

        self.dirty()
    }

    fn dirty(&mut self) {
        self.iter_views(|view, reg| {
            let mut is_dirty = false;

            iter_fields(view.as_reflect_mut(), |field| {
                if let Some(reflect_state) = reg.get_type_data::<ReflectState>(field.type_id()) {
                    let Some(state) = reflect_state.get(field) else {
                        return;
                    };

                    is_dirty = is_dirty || state.is_dirty();
                }
            });

            if is_dirty {
                view.messages();

                let build = view.build();
            }
        });
    }

    pub fn paint(&mut self, size: winit::dpi::PhysicalSize<u32>, canvas: &mut Canvas) {
        self.taffy
            .compute_layout(
                self.root,
                Size {
                    width: length(size.width as f32),
                    height: length(size.height as f32),
                },
            )
            .unwrap();

        let mut acc_point = Point { x: 0, y: 0 };
        let mut prev_parent = self.root;

        for (parent, node) in Self::iter_elements(&self.taffy, self.root) {
            let parent_layout = self.taffy.layout(parent).unwrap();

            if parent != prev_parent {
                prev_parent = parent;
                acc_point = Point {
                    x: acc_point.x + parent_layout.location.x as u32,
                    y: acc_point.y + parent_layout.location.y as u32,
                }
            }

            let layout: Layout = self.taffy.layout(node).unwrap().clone().into();

            let v = self.elements.get_mut(&node).unwrap();

            v.layout(layout.plus_location(acc_point), canvas);
            v.render(layout.plus_location(acc_point), canvas);
        }
    }

    pub fn iter_elements<'a>(
        taffy: &'a TaffyTree,
        from: NodeId,
    ) -> impl Iterator<Item = (NodeId, NodeId)> + 'a {
        struct TaffyAllIter<'a> {
            taffy: &'a TaffyTree,
            parent: NodeId,
            index: usize,
            to_process: VecDeque<NodeId>,
        }

        impl<'a> Iterator for TaffyAllIter<'a> {
            type Item = (NodeId, NodeId);

            fn next(&mut self) -> Option<Self::Item> {
                // let parent = self.taffy.parent(self.current).unwrap_or(self.current);
                // let to_return = self.current;

                if let Ok(next_child) = self.taffy.child_at_index(self.parent, self.index) {
                    self.to_process.push_back(next_child);
                    self.index += 1;

                    Some((self.parent, next_child))
                } else {
                    let Some(new_current) = self.to_process.remove(0) else {
                        return None;
                    };

                    self.parent = new_current;
                    self.index = 0;
                    self.next()
                }
            }
        }

        TaffyAllIter {
            taffy,
            parent: from,
            index: 0,
            to_process: VecDeque::with_capacity(taffy.total_node_count()),
        }
    }

    pub fn iter_views(&mut self, mut f: impl FnMut(&mut dyn View, &TypeRegistry)) {
        iter_views(&self.registry, &mut self.view, &mut f);
    }
}

fn iter_views(
    reg: &TypeRegistry,
    view: &mut dyn Reflect,
    f: &mut impl FnMut(&mut dyn View, &TypeRegistry),
) {
    let reflect_view = reg.get_type_data::<ReflectView>(view.type_id());

    match reflect_view {
        Some(reflect_view) => {
            let v: &mut dyn View = reflect_view.get_mut(view).unwrap();

            f(v, reg);

            iter_fields(v.as_reflect_mut(), |field| iter_views(reg, field, f))
        }
        None => {}
    }
}

fn iter_fields(of: &mut dyn Reflect, mut f: impl FnMut(&mut dyn Reflect)) {
    match of.reflect_mut() {
        bevy_reflect::ReflectMut::Struct(s) => {
            let mut index = 0;

            while let Some(item) = s.field_at_mut(index) {
                index += 1;

                // iter_views(reg, item, f)
                f(item)
            }
        }
        bevy_reflect::ReflectMut::Enum(e) => {
            let mut index = 0;

            while let Some(item) = e.field_at_mut(index) {
                index += 1;

                f(item)
            }
        }
        bevy_reflect::ReflectMut::TupleStruct(ts) => {
            let mut index = 0;

            while let Some(item) = ts.field_mut(index) {
                index += 1;

                f(item)
            }
        }
        bevy_reflect::ReflectMut::Value(_) => {}
        _ => {
            dbg!(of);
            todo!();
        }
    }
}

fn mount_elements(
    taffy: &mut TaffyTree,
    parent: NodeId,
    item: Element,
    store_element: &mut impl FnMut(NodeId, MountableElement),
) {
    let Element { el, children } = item;

    let id = taffy.new_leaf(el.style().0).unwrap();
    taffy.add_child(parent, id).unwrap();

    store_element(id, el);

    if let Some(children) = children {
        for child in children {
            mount_elements(taffy, id, child, store_element);
        }
    }
}
