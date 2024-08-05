use std::collections::HashMap;

use bevy_reflect::{GetPath, GetTypeRegistration, Reflect, TypeRegistry};
use taffy::{prelude::length, NodeId, Size, TaffyTree};
use winit::dpi::PhysicalSize;

use crate::{Canvas, Element, ElementTrait, Layout, Point, ReflectView, View};

pub struct App<V> {
    taffy: TaffyTree,
    registry: TypeRegistry,
    view: V,
    elements: HashMap<NodeId, Element>,
    root: NodeId,
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

        create_element_styles(
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

        self.taffy.print_tree(self.root);

        for (k, v) in self.elements.iter() {
            let parent = self.taffy.parent(*k).unwrap();
            let parent_layout = self.taffy.layout(parent).unwrap();
            let mut layout: Layout = self.taffy.layout(*k).unwrap().clone().into();
            layout.plus_location(Point {
                x: parent_layout.location.x as u32,
                y: parent_layout.location.y as u32,
            });

            v.render(layout, canvas);
        }
    }

    pub fn iter_views(&mut self, mut f: impl FnMut(&mut dyn View)) {
        iter_views(&self.registry, &mut self.view, &mut f);
    }
}

fn iter_views(reg: &TypeRegistry, view: &mut dyn Reflect, f: &mut impl FnMut(&mut dyn View)) {
    let reflect_view = reg.get_type_data::<ReflectView>(view.type_id());

    match reflect_view {
        Some(reflect_view) => {
            let v: &mut dyn View = reflect_view.get_mut(view).unwrap();

            f(v);

            match view.reflect_mut() {
                bevy_reflect::ReflectMut::Struct(s) => {
                    let mut index = 0;

                    while let Some(item) = s.field_at_mut(index) {
                        index += 1;

                        iter_views(reg, item, f)
                    }
                }
                bevy_reflect::ReflectMut::Enum(e) => {
                    let mut index = 0;

                    while let Some(item) = e.field_at_mut(index) {
                        index += 1;

                        iter_views(reg, item, f)
                    }
                }
                bevy_reflect::ReflectMut::Value(_) => {}
                _ => todo!(),
            }
        }
        None => {}
    }
}

fn create_element_styles(
    taffy: &mut TaffyTree,
    parent: NodeId,
    item: Element,
    f: &mut impl FnMut(NodeId, Element),
) {
    let (it, children) = item.consume();

    let id = taffy.new_leaf(it.style().0).unwrap();
    taffy.add_child(parent, id).unwrap();

    f(id, it);

    if let Some(children) = children {
        for child in children {
            create_element_styles(taffy, id, child, f);
        }
    }
}
