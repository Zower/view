use std::{
    collections::{HashMap, VecDeque},
    usize,
};

use bevy_reflect::{GetTypeRegistration, Reflect, TypeRegistry};
use taffy::{prelude::length, NodeId, Size, TaffyTree, TraversePartialTree};
use winit::dpi::PhysicalSize;

use crate::{
    Canvas, Element, ElementAndChildren, Layout, MountableElement, MountedElementBehaviour,
    PerChildElementThingy, Point, RebuildResult, ReflectStateTrait, View, ViewElement,
};

pub struct App<V> {
    tree: ElementTree,
    registry: TypeRegistry,
    view: V,
}

#[derive(Debug)]
pub enum AppEvent {
    Clicked(u32, u32),
}

impl<V: View + GetTypeRegistration> App<V> {
    pub fn new(view: V, size: PhysicalSize<u32>) -> Self {
        let mut type_registry = TypeRegistry::new();

        type_registry.register::<V>();

        let tree = ElementTree::create(&mut type_registry, &view);

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
                if let Some(reflect_state) = self
                    .registry
                    .get_type_data::<ReflectStateTrait>(field.type_id())
                {
                    let Some(state) = reflect_state.get_mut(field) else {
                        return;
                    };

                    if state.is_dirty() {
                        state.process();
                        is_dirty = true;
                    }
                }
            });

            if is_dirty {
                dirty_views.push(node);
            }
        }

        for dirty in dirty_views {
            self.tree.modify_if_necessary(&mut self.registry, dirty);
        }
    }

    pub fn paint(&mut self, size: winit::dpi::PhysicalSize<u32>, canvas: &mut Canvas) {
        dbg!(self.tree.taffy.total_node_count());
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

pub(crate) fn iter_fields(of: &mut dyn Reflect, mut f: impl FnMut(usize, &mut dyn Reflect)) {
    match of.reflect_mut() {
        bevy_reflect::ReflectMut::Struct(s) => {
            let mut index = 0;

            loop {
                let Some(item) = s.field_at_mut(index) else {
                    break;
                };

                f(index, item);

                index += 1;
            }
        }
        bevy_reflect::ReflectMut::Enum(e) => {
            let mut index = 0;

            while let Some(item) = e.field_at_mut(index) {
                f(index, item);

                index += 1;
            }
        }
        bevy_reflect::ReflectMut::TupleStruct(ts) => {
            let mut index = 0;

            while let Some(item) = ts.field_mut(index) {
                f(index, item);

                index += 1;
            }
        }
        bevy_reflect::ReflectMut::Value(_) => {}
        _ => {
            dbg!(of);
            todo!();
        }
    }
}

pub struct ElementTree {
    // Also holds parent, child information
    taffy: TaffyTree,
    elements: HashMap<NodeId, MountableElement>,
    root: NodeId,
}

impl ElementTree {
    pub fn create<V: View>(registry: &mut TypeRegistry, root_item: &V) -> Self {
        Self::create_internal(registry, root_item.build())
    }

    fn create_internal(registry: &mut TypeRegistry, element: impl Element) -> Self {
        let mut taffy = TaffyTree::default();
        let elements = HashMap::default();

        let root = taffy
            .new_leaf(taffy::Style {
                size: taffy::Size {
                    // todo
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

        mount_children(registry, &mut this, root, element, None);

        this
    }

    pub fn insert(&mut self, element: MountableElement, parent: NodeId) -> NodeId {
        let id = self.taffy.new_leaf(element.style().0).unwrap();
        self.taffy.add_child(parent, id).unwrap();

        self.elements.insert(id, element);

        id
    }

    pub fn insert_at(&mut self, element: MountableElement, parent: NodeId, idx: usize) -> NodeId {
        let id = self.taffy.new_leaf(element.style().0).unwrap();
        self.taffy.insert_child_at_index(parent, idx, id).unwrap();
        self.elements.insert(id, element);

        id
    }

    pub fn modify_if_necessary(&mut self, registry: &mut TypeRegistry, changed: NodeId) {
        self.comp_exchange(changed, registry);
    }

    fn comp_exchange(&mut self, view_id: NodeId, registry: &mut TypeRegistry) {
        debug_assert!(self.taffy.child_count(view_id) == 1);
        let only_child = self.taffy.child_at_index(view_id, 0).unwrap();

        let Some(MountableElement::View(ViewElement(view))) = self.elements.remove(&view_id) else {
            panic!()
        };

        view.dyn_cmp(only_child, self, registry);

        // todo avoid this by passing in tree?
        self.elements.insert(view_id, ViewElement(view).into());

        // iter_elements_cmp(self, only_child, new_element, registry);
    }
}

pub fn iter_elements_cmp(
    tree: &mut ElementTree,
    processing: NodeId,
    mut new_element_at_position: impl Element,
    registry: &mut TypeRegistry,
) {
    let ElementTree {
        taffy,
        elements,
        root,
    } = tree;

    // let new_mountable_element = &mut new_element_at_position.el;

    let element_at_current_position = elements.remove(&processing).unwrap();

    // if mem::discriminant(&element_at_current_position)
    //     != mem::discriminant(&new_mountable_element)
    // {
    //     let parent = taffy.parent(processing).unwrap();
    //     let to_delete = iter_elements(&taffy, processing)
    //         .map(|it| it.1)
    //         .collect::<Vec<_>>();

    //     let mut idx = 0;

    //     while let false = taffy.child_at_index(parent, idx).unwrap() == processing {
    //         idx += 1;
    //     }

    //     taffy.remove(processing).unwrap();

    //     for to_delete in to_delete {
    //         elements.remove(&to_delete).unwrap();
    //         let _ = taffy.remove(to_delete);
    //     }

    //     // todo this will lead to wrong position
    //     mount_children(registry, tree, parent, new_element_at_position, Some(idx));

    //     return;
    // }

    if let RebuildResult::Replace =
        new_element_at_position.try_reuse(element_at_current_position, registry)
    {
        let parent = taffy.parent(processing).unwrap();
        let to_delete = iter_elements(&taffy, processing)
            .map(|it| it.1)
            .collect::<Vec<_>>();

        for to_delete in to_delete {
            elements.remove(&to_delete).unwrap();
            let _ = taffy.remove(to_delete);
        }

        let mut idx = 0;

        while let false = taffy.child_at_index(parent, idx).unwrap() == processing {
            idx += 1;
        }

        taffy.remove(processing).unwrap();

        // todo this will lead to wrong position not anymore i thinks
        mount_children(registry, tree, parent, new_element_at_position, Some(idx));
    } else {
        struct TheBuilderMagicThingBoo<'a> {
            tree: &'a mut ElementTree,
            processing: NodeId,
            registry: &'a mut TypeRegistry,
            child_idx: usize,
        }

        impl<'a> PerChildElementThingy for TheBuilderMagicThingBoo<'a> {
            fn dothething<E: Element>(&mut self, e: E, parent: NodeId) {
                debug_assert!(self.processing == parent);

                iter_elements_cmp(
                    self.tree,
                    self.tree
                        .taffy
                        .child_at_index(parent, self.child_idx)
                        .unwrap(),
                    e,
                    self.registry,
                );

                self.child_idx += 1;
            }

            fn insert(&mut self, el: MountableElement) -> NodeId {
                self.tree.elements.insert(self.processing, el);

                self.processing
            }

            fn registry(&mut self) -> &mut TypeRegistry {
                &mut self.registry
            }
        }

        new_element_at_position.insert_perform_per_child(TheBuilderMagicThingBoo {
            tree,
            processing,
            registry,
            child_idx: 0,
        });

        // todo update style??
        // new_element_at_position.cmp()
        // elements.insert(processing, new_element_at_position.el);

        // if let Some(children) = new_element_at_position.children {
        //     for (idx, child) in children.into_iter().enumerate() {
        //         let processing = tree.taffy.child_at_index(processing, idx).unwrap();
        //         iter_elements_cmp(tree, processing, child, registry);
        //     }
        // }
    }
}

pub(crate) fn mount_children<T: Element>(
    registry: &mut TypeRegistry,
    tree: &mut ElementTree,
    parent: NodeId,
    element: T,
    idx: Option<usize>,
) {
    // let Element {
    //     mut el,
    //     mut children,
    // } = element;
    // let (mut eL, children) = element.consume();
    struct Mounter<'a> {
        tree: &'a mut ElementTree,
        parent: NodeId,
        registry: &'a mut TypeRegistry,
        idx: Option<usize>,
    }

    impl<'a> PerChildElementThingy for Mounter<'a> {
        fn dothething<E: Element>(&mut self, e: E, parent: NodeId) {
            mount_children(&mut self.registry, self.tree, parent, e, self.idx)
        }

        fn insert(&mut self, el: MountableElement) -> NodeId {
            if let Some(idx) = self.idx {
                self.tree.insert_at(el, self.parent, idx)
            } else {
                self.tree.insert(el, self.parent)
            }
        }

        fn registry(&mut self) -> &mut TypeRegistry {
            &mut self.registry
        }
    }

    element.insert_perform_per_child(Mounter {
        tree,
        parent,
        registry,
        idx,
    })

    // element.(registry, tree, parent, idx);

    // TODO?

    // todo!();
    // if let mountableElement::View(view) = &mut el {
    //     view.0.register(registry);

    //     iter_fields(view.0.as_reflect_mut(), |_, field| {
    //         if let Some(reflect_state) =
    //             registry.get_type_data::<ReflectStateTrait>(field.type_id())
    //         {
    //             let Some(state) = reflect_state.get_mut(field) else {
    //                 return;
    //             };

    //             state.init()
    //         }
    //     });

    //     // children = Some(vec![view.0.build()]);
    // }

    // let id = if let Some(idx) = idx {
    //     // todo not into?
    //     tree.insert_at(el.into(), parent, idx)
    // } else {
    //     tree.insert(el.into(), parent)
    // };

    // T::convert(children, |child| {
    //     mount_children(registry, tree, id, child, None);
    // });
}
