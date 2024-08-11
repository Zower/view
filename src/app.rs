use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
};

use bevy_reflect::{Access, GetPath, GetTypeRegistration, ParsedPath, Reflect, TypeRegistry};
use taffy::{prelude::length, NodeId, PrintTree, Size, TaffyTree};
use winit::dpi::PhysicalSize;

use crate::{
    Canvas, Element, Layout, MountableElement, MountedElementBehaviour, Point, ReflectState,
    ReflectView, View,
};

pub struct App<V> {
    tree: ElementTree,
    registry: TypeRegistry,
    view: V,
    view_data: ViewMetaData,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ViewId(ParsedPath);

#[derive(Debug)]
pub enum AppEvent {
    Clicked(u32, u32),
}

impl<V: View + GetTypeRegistration + GetPath> App<V> {
    pub fn new(view: V, size: PhysicalSize<u32>) -> Self {
        let mut type_registry = TypeRegistry::new();

        type_registry.register::<V>();

        let mut view_data = Default::default();

        let tree = ElementTree::create(&type_registry, &view, &mut view_data);

        Self {
            registry: type_registry,
            tree,
            view,
            view_data,
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

        iter_views(&self.registry, &mut self.view, &mut |view, reg, access| {
            let mut is_dirty = false;

            iter_fields(view.as_reflect_mut(), |_, field| {
                if let Some(reflect_state) = reg.get_type_data::<ReflectState>(field.type_id()) {
                    let Some(state) = reflect_state.get(field) else {
                        return;
                    };

                    if state.is_dirty() {
                        dbg!(&field);
                    }

                    is_dirty = is_dirty || state.is_dirty();
                }
            });

            if is_dirty {
                dirty_views.push(ViewId(access.clone().into()));
            }
        });

        for dirty in dirty_views {
            let view = self.view.reflect_path_mut(&dirty.0).unwrap();

            let view = reflect_view_mut_or_panic(&self.registry, view);

            view.messages();

            self.tree.modify_if_necessary(
                &self.registry,
                &self.view,
                self.view_data.element_created_by_view(&dirty),
                dirty,
                &mut self.view_data,
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

fn mount_children<V: View + Reflect + GetPath>(
    tree: &mut ElementTree,
    reg: &TypeRegistry,
    parent: NodeId,
    root_view: &V,
    element: Element,
    first_child_of_view: bool,
    view: &mut ViewId,
    view_data: &mut ViewMetaData,
) {
    let Element { el, children } = element;

    let id = tree.insert(el, parent);

    if first_child_of_view {
        view_data.create_with_first_child(view.clone(), id);
    } else {
        view_data.add_child(view.clone(), id);
    }

    if let Some(children) = children {
        for child in children {
            match child {
                crate::ElementOrPath::Element(element) => {
                    mount_children(tree, reg, id, root_view, element, false, view, view_data)
                }
                crate::ElementOrPath::Path(access) => {
                    view.0 .0.extend(access.0);

                    let field = {
                        let mut temp_vec = vec![];

                        std::mem::swap(&mut temp_vec, &mut view.0 .0);

                        let mut temp = ParsedPath(temp_vec);
                        dbg!(&temp);
                        let field = root_view.reflect_path(&temp).unwrap();

                        std::mem::swap(&mut view.0 .0, &mut temp.0);

                        field
                    };

                    let element = reflect_view_or_panic(reg, field).build();

                    mount_children(tree, reg, id, root_view, element, true, view, view_data);
                }
            }
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
    pub fn create<V: View>(
        reg: &TypeRegistry,
        root_item: &V,
        view_data: &mut ViewMetaData,
    ) -> Self {
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

        mount_children(
            &mut this,
            reg,
            root,
            root_item,
            root_item.build(),
            true,
            &mut ViewId(ParsedPath(Vec::new())),
            view_data,
        );

        this
    }

    pub fn insert(&mut self, element: MountableElement, parent: NodeId) -> NodeId {
        let id = self.taffy.new_leaf(element.style().0).unwrap();
        self.taffy.add_child(parent, id).unwrap();

        self.elements.insert(id, element);

        id
    }

    pub fn modify_if_necessary<V: View>(
        &mut self,
        reg: &TypeRegistry,
        root_item: &V,
        from: NodeId,
        mut view_id: ViewId,
        view_meta_data: &mut ViewMetaData,
    ) {
        dbg!(self.taffy.total_node_count());
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

        let mut new_meta_data = ViewMetaData::new();

        let item = root_item.reflect_path(&view_id.0).unwrap();

        let view = reflect_view_or_panic(reg, item);

        let mut new = Self {
            taffy,
            elements,
            root,
        };

        mount_children(
            &mut new,
            reg,
            root,
            root_item,
            view.build(),
            true,
            &mut view_id,
            &mut new_meta_data,
        );

        if !self.eq(from, &new) {
            self.mount(from, new, view_meta_data, new_meta_data);
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

    fn remove_node(&mut self, node: NodeId, meta_data: &mut ViewMetaData) {
        self.elements.remove(&node).unwrap();
        let _ = self.taffy.remove(node);
        meta_data.remove_element(node);
    }

    fn mount(
        &mut self,
        from: NodeId,
        other: ElementTree,
        real_meta_data: &mut ViewMetaData,
        new_meta_data: ViewMetaData,
    ) {
        let to_delete = iter_elements(&self.taffy, from)
            .map(|it| it.1)
            .collect::<Vec<_>>();

        for to_delete in to_delete {
            self.remove_node(to_delete, real_meta_data);
        }

        let ElementTree {
            taffy,
            mut elements,
            root,
        } = other;

        let mut old_to_new: HashMap<NodeId, NodeId> = Default::default();

        let other_real_root = taffy.child_at_index(root, 0).unwrap();
        old_to_new.insert(other_real_root, from);

        for (old_parent, to_create) in iter_elements(&taffy, other_real_root) {
            let element = elements.remove(&to_create).unwrap();
            let new = self.taffy.new_leaf(element.style().0).unwrap();

            old_to_new.insert(to_create, new);

            let parent = old_to_new[&old_parent];

            self.elements.insert(new, element);
            self.taffy.add_child(parent, new).unwrap();
        }

        real_meta_data.copy(new_meta_data, old_to_new);
    }
}

#[derive(Default)]
pub struct ViewMetaData {
    view_to_element: HashMap<ViewId, NodeId>,
    element_to_view: HashMap<NodeId, ViewId>,
}

impl ViewMetaData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_with_first_child(&mut self, mut id: ViewId, element_id: NodeId) {
        // Ensure keys map correctly
        for offset_access in &mut id.0 .0 {
            offset_access.offset = None;
        }

        self.view_to_element.insert(id.clone(), element_id);
        self.element_to_view.insert(element_id, id);
    }

    pub fn add_child(&mut self, mut parent: ViewId, element_id: NodeId) {
        // Ensure keys map correctly
        for offset_access in &mut parent.0 .0 {
            offset_access.offset = None;
        }
        self.element_to_view.insert(element_id, parent);
    }

    pub fn element_created_by_view(&self, id: &ViewId) -> NodeId {
        self.view_to_element[id]
    }

    pub fn remove_element(&mut self, element_id: NodeId) {
        let Some(key) = self.element_to_view.remove(&element_id) else {
            return;
        };

        self.view_to_element.remove(&key);
    }

    pub fn copy(&mut self, other: ViewMetaData, node_mapping: HashMap<NodeId, NodeId>) {
        self.view_to_element.extend(
            other
                .view_to_element
                .into_iter()
                .map(|(k, v)| (k, node_mapping[&v])),
        );
        self.element_to_view.extend(
            other
                .element_to_view
                .into_iter()
                .map(|(k, v)| (node_mapping[&k], v)),
        );
    }
}
