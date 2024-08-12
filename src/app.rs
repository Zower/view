use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
    mem,
    ops::Deref,
};

use bevy_reflect::{GetTypeRegistration, ParsedPath, Reflect, TypeRegistry};
use taffy::{prelude::length, NodeId, Size, TaffyTree, TraversePartialTree};
use winit::dpi::PhysicalSize;

use crate::{
    Canvas, Element, Layout, MountableElement, MountedElementBehaviour, Point, ReflectState,
    ReflectView, View, ViewElement,
};

pub struct App<V> {
    tree: ElementTree,
    registry: TypeRegistry,
    view: V,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ViewId(ParsedPath);

#[derive(Debug)]
pub enum AppEvent {
    Clicked(u32, u32),
}

impl<V: View + GetTypeRegistration> App<V> {
    pub fn new(view: V, size: PhysicalSize<u32>) -> Self {
        let mut type_registry = TypeRegistry::new();

        type_registry.register::<V>();

        let tree = ElementTree::create(&view);

        Self {
            registry: type_registry,
            tree,
            view,
        }
    }
}

impl<V: View> App<V> {
    pub fn event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Clicked(x, y) => {
                for (_, node) in iter_elements(&self.tree.taffy, self.tree.root) {
                    let el = self.tree.elements.get_mut(&node).unwrap();
                    let layout: Layout = self.tree.taffy.layout(node).unwrap().clone().into();
                    let MountableElement::Button(_) = el else {
                        continue;
                    };

                    if layout.location.x < x
                        && layout.location.y < y
                        && x < layout.location.x + layout.size.width
                        && y < layout.location.y + layout.size.height
                    {
                        el.event(crate::ElementEvent::Click(x, y));
                    }
                }
            }
        }

        self.dirty()
    }

    fn dirty(&mut self) {
        let mut dirty_views = vec![];

        for (_, node) in iter_elements(&self.tree.taffy, self.tree.root) {
            let MountableElement::View(ViewElement(view)) =
                self.tree.elements.get_mut(&node).unwrap()
            else {
                continue;
            };

            let mut is_dirty = false;

            iter_fields(view.as_reflect_mut(), |_, field| {
                if let Some(reflect_state) =
                    self.registry.get_type_data::<ReflectState>(field.type_id())
                {
                    let Some(state) = reflect_state.get(field) else {
                        return;
                    };

                    is_dirty = is_dirty || state.is_dirty();
                }
            });

            if is_dirty {
                view.messages();
                dirty_views.push(node);
            }
        }

        for dirty in dirty_views {
            self.tree.modify_if_necessary(dirty);
        }
    }

    pub fn paint(&mut self, size: winit::dpi::PhysicalSize<u32>, canvas: &mut Canvas) {
        self.tree
            .taffy
            .compute_layout(
                self.tree.root,
                Size {
                    width: length(size.width as f32),
                    height: length(size.height as f32),
                },
            )
            .unwrap();

        let mut acc_point = Point { x: 0, y: 0 };
        let mut prev_parent = self.tree.root;

        for (parent, node) in iter_elements(&self.tree.taffy, self.tree.root) {
            let parent_layout = self.tree.taffy.layout(parent).unwrap();

            if parent != prev_parent {
                prev_parent = parent;
                acc_point = Point {
                    x: acc_point.x + parent_layout.location.x as u32,
                    y: acc_point.y + parent_layout.location.y as u32,
                }
            }

            let layout: Layout = self.tree.taffy.layout(node).unwrap().clone().into();

            let v = self.tree.elements.get_mut(&node).unwrap();

            v.layout(layout.plus_location(acc_point), canvas);
            v.render(layout.plus_location(acc_point), canvas);
        }
    }
}

fn iter_views(tree: &ElementTree, from: NodeId) -> impl Iterator<Item = &dyn View> {
    fn what(b: &Box<dyn View>) -> &dyn View {
        let b_ref: &dyn View = b.deref();

        b_ref
    }

    iter_elements(&tree.taffy, from).filter_map(|it| match &tree.elements[&it.1] {
        MountableElement::View(view) => Some(what(&view.0)),
        _ => None,
    })
}

fn iter_elements<'a>(
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

fn iter_fields(of: &mut dyn Reflect, mut f: impl FnMut(&str, &mut dyn Reflect)) {
    match of.reflect_mut() {
        bevy_reflect::ReflectMut::Struct(s) => {
            let mut index = 0;

            loop {
                let name = s.name_at(index).map(|it| it.to_owned());
                let Some(item) = s.field_at_mut(index) else {
                    break;
                };
                index += 1;

                // iter_views(reg, item, f)
                f(&name.unwrap(), item);
            }
        }
        bevy_reflect::ReflectMut::Enum(e) => {
            let mut index = 0;

            while let Some(item) = e.field_at_mut(index) {
                let str = format!(".{}", index);

                index += 1;

                f(&str, item)
            }
        }
        bevy_reflect::ReflectMut::TupleStruct(ts) => {
            let mut index = 0;

            while let Some(item) = ts.field_mut(index) {
                index += 1;

                f(&format!(".{}", index), item)
            }
        }
        bevy_reflect::ReflectMut::Value(_) => {}
        _ => {
            dbg!(of);
            todo!();
        }
    }
}

fn mount_children(tree: &mut ElementTree, parent: NodeId, element: Element) {
    let Element { el, children } = element;

    let id = tree.insert(el, parent);

    if let Some(children) = children {
        for child in children {
            mount_children(tree, id, child);
        }
    }
}

fn reflect_view_or_panic<'a>(registry: &TypeRegistry, view: &'a dyn Reflect) -> &'a dyn View {
    let reflect_view = registry
        .get_type_data::<ReflectView>(view.type_id())
        .unwrap();
    reflect_view.get(view).unwrap()
}

fn reflect_view_mut_or_panic<'a>(
    registry: &TypeRegistry,
    view: &'a mut dyn Reflect,
) -> &'a mut dyn View {
    let reflect_view = registry
        .get_type_data::<ReflectView>(view.type_id())
        .unwrap();
    reflect_view.get_mut(view).unwrap()
}

struct ElementTree {
    // Also holds parent, child information
    taffy: TaffyTree,
    elements: HashMap<NodeId, MountableElement>,
    root: NodeId,
}

impl ElementTree {
    pub fn create<V: View>(root_item: &V) -> Self {
        let mut taffy = TaffyTree::default();
        let elements = HashMap::default();

        let root = taffy
            .new_leaf(taffy::Style {
                size: taffy::Size {
                    width: length(800.0),
                    height: length(800.0),
                },
                ..Default::default()
            })
            .unwrap();

        let mut this = Self {
            taffy,
            elements,
            root,
        };

        mount_children(&mut this, root, root_item.build());

        this
    }

    pub fn insert(&mut self, element: MountableElement, parent: NodeId) -> NodeId {
        let id = self.taffy.new_leaf(element.style().0).unwrap();
        self.taffy.add_child(parent, id).unwrap();

        self.elements.insert(id, element);

        id
    }

    pub fn modify_if_necessary(&mut self, changed: NodeId) {
        let Some(MountableElement::View(ViewElement(view))) = self.elements.get(&changed) else {
            panic!()
        };

        dbg!(self.taffy.total_node_count());
        let mut taffy = TaffyTree::default();
        let mut elements = HashMap::default();

        let it = view.build();

        let Element { el, children } = it;

        let id = taffy.new_leaf(el.style().0).unwrap();
        elements.insert(id, el);

        let mut new = Self {
            taffy,
            elements,
            root: id,
        };

        if let Some(children) = children {
            for child in children {
                mount_children(&mut new, id, child);
            }
        }

        self.comp_exchange(changed, new)
    }

    fn comp_exchange(&mut self, from: NodeId, mut other: ElementTree) {
        let mut prev_parent = from;
        let mut other_prev_parent = other.root;

        for (idx, child) in self.taffy.child_ids(prev_parent).enumerate() {
            let Ok(other_child) = other.taffy.child_at_index(other_prev_parent, idx) else {
                todo!()
            };
        }
        // let should_just_remount = 'a: {
        //     for ((parent, node), (other_parent, other_node)) in
        //         iter_elements(&self.taffy, from).zip(iter_elements(&other.taffy, other.root))
        //     {
        //         // Make sure both parents change at the same time (same number of children)
        //         if prev_parent != parent {
        //             if other_parent == other_prev_parent {
        //                 break 'a true;
        //             } else {
        //                 prev_parent = parent;
        //                 other_prev_parent = other_parent
        //             }
        //         }

        //         let element = self.elements.get_mut(&node).unwrap();
        //         let other_element = other.elements.remove(&other_node).unwrap();

        //         if mem::discriminant(element) != mem::discriminant(&other_element) {
        //             other.elements.insert(other_node, other_element);
        //             break 'a true;
        //         }

        //         if let crate::RebuildResult::Replace(unused_element) =
        //             element.try_rebuild(other_element)
        //         {
        //             other.elements.insert(other_node, unused_element);
        //             break 'a true;
        //         }
        //     }

        //     false
        // };

        // if should_just_remount {
        //     self.mount(from, other);
        // }
    }

    fn remove_node(&mut self, node: NodeId) {
        self.elements.remove(&node).unwrap();
        let _ = self.taffy.remove(node);
    }

    fn mount(&mut self, from: NodeId, other: ElementTree, from_other: NodeId) {
        let to_delete = iter_elements(&self.taffy, from)
            .map(|it| it.1)
            .collect::<Vec<_>>();

        for to_delete in to_delete {
            self.remove_node(to_delete);
        }

        let ElementTree {
            taffy: new_taffy,
            elements: mut new_elements,
            root: other_root,
        } = other;

        let mut old_to_new: HashMap<NodeId, NodeId> = Default::default();

        old_to_new.insert(other_root, from);

        for (old_parent, to_create) in iter_elements(&new_taffy, from_other) {
            let element = new_elements.remove(&to_create).unwrap();
            let new = self.taffy.new_leaf(element.style().0).unwrap();

            old_to_new.insert(to_create, new);

            let parent = old_to_new[&old_parent];

            self.elements.insert(new, element);
            self.taffy.add_child(parent, new).unwrap();
        }
    }
}
