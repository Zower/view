use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
};

use bevy_reflect::{Access, GetPath, GetTypeRegistration, ParsedPath, Reflect, TypeRegistry};
use taffy::{prelude::length, NodeId, Size, TaffyTree};
use winit::dpi::PhysicalSize;

use crate::{
    Canvas, Element, Layout, MountableElement, MountedElementBehaviour, Point, ReflectState,
    ReflectView, View,
};

pub struct App<V> {
    tree: ElementTree,
    registry: TypeRegistry,
    view: V,
    view_created_element: HashMap<ViewId, NodeId>,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct ViewId(ParsedPath);

#[derive(Debug)]
pub enum AppEvent {
    Clicked(u32, u32),
}

impl<V: View + GetTypeRegistration + GetPath> App<V> {
    pub fn new(view: V, size: PhysicalSize<u32>) -> Self {
        let mut type_registry = TypeRegistry::new();

        type_registry.register::<V>();

        let mut view_created_element = Default::default();

        let tree = ElementTree::create(&type_registry, &view, &mut view_created_element);

        Self {
            registry: type_registry,
            tree,
            view,
            view_created_element,
        }
    }
}

impl<V: View> App<V> {
    pub fn event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Clicked(x, y) => {
                for (_, node) in iter_elements(&self.tree.taffy, self.tree.root) {
                    let el = self.tree.elements.get_mut(&node).unwrap();

                    el.event(crate::ElementEvent::Click(x, y));
                }
            }
        }

        self.dirty()
    }

    fn dirty(&mut self) {
        let mut dirty_views = vec![];

        iter_views(&self.registry, &mut self.view, &mut |view, reg, access| {
            let mut is_dirty = false;

            iter_fields(view.as_reflect_mut(), |path, field| {
                if let Some(reflect_state) = reg.get_type_data::<ReflectState>(field.type_id()) {
                    let Some(state) = reflect_state.get(field) else {
                        return;
                    };

                    is_dirty = is_dirty || state.is_dirty();
                }
            });

            if is_dirty {
                dirty_views.push(ViewId(access.clone().into()));

                // let root = self
                //     .taffy
                //     .new_leaf(taffy::Style {
                //         size: taffy::Size {
                //             width: length(800 as f32),
                //             height: length(600 as f32),
                //         },
                //         ..Default::default()
                //     })
                //     .unwrap();

                // self.root = root;

                // mount_elements(&mut self.taffy, self.root, build, &mut |id, item| {
                //     self.elements.insert(id, item);
                // });
            }
        });

        for dirty in dirty_views {
            let view = self.view.reflect_path_mut(&dirty.0).unwrap();
            let reflect_view = self
                .registry
                .get_type_data::<ReflectView>(view.type_id())
                .unwrap();

            let view = reflect_view.get_mut(view).unwrap();

            view.messages();

            self.tree.modify_if_necessary(
                &self.registry,
                &self.view,
                self.view_created_element[&dirty],
                dirty.0 .0.iter().map(|it| it.access.clone()).collect(),
                &mut self.view_created_element,
            );
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

    pub fn iter_views(&mut self, mut f: impl FnMut(&mut dyn View, &TypeRegistry, &Vec<Access>)) {
        iter_views(&self.registry, &mut self.view, &mut f);
    }
}

fn iter_views(
    reg: &TypeRegistry,
    view: &mut dyn Reflect,
    f: &mut impl FnMut(&mut dyn View, &TypeRegistry, &Vec<Access<'static>>),
) {
    iter_view_internal(&mut vec![], reg, view, f);

    fn iter_view_internal(
        accesses: &mut Vec<Access<'static>>,
        reg: &TypeRegistry,
        view: &mut dyn Reflect,
        f: &mut impl FnMut(&mut dyn View, &TypeRegistry, &Vec<Access<'static>>),
    ) {
        let reflect_view = reg.get_type_data::<ReflectView>(view.type_id());

        match reflect_view {
            Some(reflect_view) => {
                let v: &mut dyn View = reflect_view.get_mut(view).unwrap();

                f(v, reg, &accesses);

                iter_fields(v.as_reflect_mut(), |path, field| {
                    accesses.extend(
                        ParsedPath::parse(path)
                            .unwrap()
                            .0
                            .into_iter()
                            .map(|it| it.access),
                    );

                    iter_view_internal(accesses, reg, field, f)
                })
            }
            None => {}
        }
    }
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
            panic!();
            // let mut index = 0;

            // while let Some(item) = e.field_at_mut(index) {
            //     index += 1;

            //     f(item)
            // }
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

fn mount_children<V: View + Reflect + GetPath>(
    taffy: &mut TaffyTree,
    reg: &TypeRegistry,
    parent: NodeId,
    root_item: &V,
    element: Element,
    first_item: bool,
    paths: &mut Vec<Access<'static>>,
    elements: &mut HashMap<NodeId, MountableElement>,
    view_created_element: &mut HashMap<ViewId, NodeId>,
) {
    let Element { el, children } = element;

    let id = taffy.new_leaf(el.style().0).unwrap();
    taffy.add_child(parent, id).unwrap();

    elements.insert(id, el);

    if first_item {
        view_created_element.insert(ViewId(paths.clone().into()), id);
    }

    if let Some(children) = children {
        for child in children {
            match child {
                crate::ElementOrPath::Element(element) => mount_children(
                    taffy,
                    reg,
                    id,
                    root_item,
                    element,
                    false,
                    paths,
                    elements,
                    view_created_element,
                ),
                crate::ElementOrPath::Path(path) => {
                    paths.extend(path.0.into_iter().map(|it| it.access));

                    let field = root_item
                        .reflect_path(&ParsedPath::from(paths.clone()))
                        .unwrap();

                    if let Some(reflect_view) = reg.get_type_data::<ReflectView>(field.type_id()) {
                        let Some(view) = reflect_view.get(field) else {
                            panic!("None view path");
                        };

                        mount_children(
                            taffy,
                            reg,
                            id,
                            root_item,
                            view.build(),
                            true,
                            paths,
                            elements,
                            view_created_element,
                        );
                    }
                }
            }
            // mount_elements(taffy, id, child, store_element);
        }
    }
}

struct ElementTree {
    // Also holds parent, child information
    taffy: TaffyTree,
    elements: HashMap<NodeId, MountableElement>,
    root: NodeId,
}

impl ElementTree {
    pub fn create<V: View>(
        reg: &TypeRegistry,
        root_item: &V,
        view_created_element: &mut HashMap<ViewId, NodeId>,
    ) -> Self {
        let mut taffy = TaffyTree::default();
        let mut elements = HashMap::default();

        let root = taffy
            .new_leaf(taffy::Style {
                size: taffy::Size {
                    width: length(800.0),
                    height: length(800.0),
                },
                ..Default::default()
            })
            .unwrap();

        mount_children(
            &mut taffy,
            reg,
            root,
            root_item,
            root_item.build(),
            true,
            &mut vec![],
            &mut elements,
            view_created_element,
        );

        Self {
            taffy,
            elements,
            root,
        }
    }

    pub fn modify_if_necessary<V: View>(
        &mut self,
        reg: &TypeRegistry,
        root_item: &V,
        from: NodeId,
        mut paths: Vec<Access<'static>>,
        created_by: &mut HashMap<ViewId, NodeId>,
    ) {
        let mut taffy = TaffyTree::default();
        let mut elements = HashMap::default();

        let root = taffy
            .new_leaf(taffy::Style {
                size: taffy::Size {
                    width: length(800.0),
                    height: length(800.0),
                },
                ..Default::default()
            })
            .unwrap();

        let mut new_created_by = HashMap::new();

        let item = root_item
            .reflect_path(&ParsedPath::from(paths.clone()))
            .unwrap();

        let reflect_view = reg.get_type_data::<ReflectView>(item.type_id()).unwrap();
        let view = reflect_view.get(item).unwrap();

        mount_children(
            &mut taffy,
            reg,
            root,
            root_item,
            view.build(),
            true,
            &mut paths,
            &mut elements,
            &mut new_created_by,
        );

        let new = Self {
            taffy,
            elements,
            root,
        };

        if !self.eq(from, &new) {
            self.mount(from, new, created_by, new_created_by);
        }
    }

    fn eq(&self, from: NodeId, other: &ElementTree) -> bool {
        let other_real_root = other.taffy.child_at_index(other.root, 0).unwrap();

        let mut prev_parent = from;
        let mut other_prev_parent = other_real_root;

        for ((parent, node), (other_parent, other_node)) in
            iter_elements(&self.taffy, from).zip(iter_elements(&other.taffy, other_real_root))
        {
            // Make sure both parents change at the same time (same number of children)
            if prev_parent != parent {
                if other_parent == other_prev_parent {
                    return false;
                } else {
                    prev_parent = parent;
                    other_prev_parent = other_parent
                }
            }

            let element = &self.elements[&node];
            let other_element = &other.elements[&other_node];

            if element.needs_rebuild(other_element) {
                return false;
            }
        }

        true
    }

    fn mount(
        &mut self,
        from: NodeId,
        other: ElementTree,
        real_created_by: &mut HashMap<ViewId, NodeId>,
        new_created_by: HashMap<ViewId, NodeId>,
    ) {
        let to_delete = iter_elements(&self.taffy, from)
            .map(|it| it.1)
            .collect::<Vec<_>>();

        for to_delete in to_delete {
            self.taffy.remove(to_delete).unwrap();
            self.elements.remove(&to_delete).unwrap();

            // wow bad o n2
            if let Some(key) = real_created_by
                .iter()
                .find(|it| *it.1 == to_delete)
                .map(|it| it.0 .0.clone())
            {
                real_created_by.remove(&ViewId(key));
            }
        }

        let ElementTree {
            taffy,
            mut elements,
            root,
        } = other;

        let mut old_to_new: HashMap<NodeId, NodeId> = Default::default();
        old_to_new.insert(root, from);

        for (old_parent, to_create) in iter_elements(&taffy, root) {
            let element = elements.remove(&to_create).unwrap();
            let style = taffy.style(to_create).unwrap().clone();
            let new = self.taffy.new_leaf(style).unwrap();
            old_to_new.insert(to_create, new);
            self.elements.insert(new, element);
            let parent = old_to_new[&old_parent];
            self.taffy.add_child(parent, new).unwrap();
        }

        for new in new_created_by {
            real_created_by.insert(new.0, old_to_new[&new.1]);
        }
    }
}
