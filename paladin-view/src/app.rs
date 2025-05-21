use std::{
    collections::{HashMap, VecDeque},
    usize,
};

use bevy_reflect::{Reflect, TypeRegistry};
use taffy::{prelude::length, NodeId, Size, TaffyTree, TraversePartialTree};
use winit::dpi::PhysicalSize;

use crate::{
    BuildResult, Canvas, Element, InsertChildren, InsertContext, KeyEvent, Layout, MountedWidget,
    Point, RebuildChildren, RebuildContext, ReflectStateTrait, View, Widget,
};

pub(crate) struct App {
    tree: WidgetTree,
    registry: TypeRegistry,
}

// Global events passed through from the event loop abstraction.
#[derive(Debug)]
#[doc(hidden)]
pub(crate) enum AppEvent {
    Resize(PhysicalSize<u32>),
    Clicked(u32, u32),
    Key(KeyEvent),
    Paint(PhysicalSize<u32>),
}

impl App {
    pub(crate) fn new<V: View>(view: V, size: PhysicalSize<u32>) -> Self {
        let mut type_registry = TypeRegistry::new();

        view.register(&mut type_registry);

        let tree = WidgetTree::create(&mut type_registry, view, size);

        Self {
            registry: type_registry,
            tree,
        }
    }
}

impl App {
    pub(crate) fn event(&mut self, event: AppEvent, canvas: &mut Canvas) {
        match event {
            AppEvent::Clicked(x, y) => {
                for (_, node) in iter_elements_from(&self.tree.taffy, self.tree.root) {
                    let el = self.tree.widgets.get_mut(&node).unwrap();
                    let layout: Layout = self.tree.taffy.layout(node).unwrap().clone().into();
                    let MountedWidget::Button(_) = el else {
                        continue;
                    };

                    if layout.location.x < x
                        && layout.location.y < y
                        && x < layout.location.x + layout.size.width
                        && y < layout.location.y + layout.size.height
                    {
                        el.event(crate::WidgetEvent::Click(x, y));
                    }
                }
            }
            AppEvent::Resize(new_size) => {
                self.tree
                    .taffy
                    .set_style(
                        self.tree.root,
                        taffy::Style {
                            size: taffy::Size {
                                // todo
                                width: length(new_size.width as f32),
                                height: length(new_size.height as f32),
                            },
                            ..Default::default()
                        },
                    )
                    .expect("Root doesn't exist")
            }
            AppEvent::Paint(size) => self.paint(size, canvas),
            AppEvent::Key(key_event) => {
                for (_, node) in iter_elements_from(&self.tree.taffy, self.tree.root) {
                    let el = self.tree.widgets.get_mut(&node).unwrap();
                    let layout: Layout = self.tree.taffy.layout(node).unwrap().clone().into();
                    let MountedWidget::Button(_) = el else {
                        continue;
                    };

                    el.event(crate::WidgetEvent::Key(key_event.clone()));
                }
            }
        }

        self.dirty()
    }

    pub(crate) fn hint_dirty(&mut self, hint: NodeId) {
        let mut dirty_views = vec![];

        // iter_elements doesnt include the node itself
        let from = self.tree.taffy.parent(hint).unwrap_or(hint);

        for (_, node) in iter_elements_from(&self.tree.taffy, from) {
            let Some(MountedView(view)) = self.tree.views.get_mut(&node) else {
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

    fn dirty(&mut self) {
        self.hint_dirty(self.tree.root);
    }

    fn paint(&mut self, size: winit::dpi::PhysicalSize<u32>, canvas: &mut Canvas) {
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

        for (parent, node) in iter_elements_from(&self.tree.taffy, self.tree.root) {
            let parent_layout = self.tree.taffy.layout(parent).unwrap();

            if parent != prev_parent {
                prev_parent = parent;
                acc_point = Point {
                    x: acc_point.x + parent_layout.location.x as u32,
                    y: acc_point.y + parent_layout.location.y as u32,
                }
            }

            let layout: Layout = self.tree.taffy.layout(node).unwrap().clone().into();

            let v = self.tree.widgets.get_mut(&node).unwrap();

            v.layout(layout.plus_location(acc_point), canvas.font_system());
            v.render(layout.plus_location(acc_point), canvas);
        }
    }
}

fn iter_elements_from<'a>(
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

struct MountedView(Box<dyn View>);

// Should only be used by DynView
#[doc(hidden)]
pub struct WidgetTree {
    // Also holds parent, child information
    taffy: TaffyTree,
    widgets: HashMap<NodeId, MountedWidget>,
    views: HashMap<NodeId, MountedView>,
    root: NodeId,
}

impl WidgetTree {
    pub(crate) fn create<V: View>(
        registry: &mut TypeRegistry,
        root_item: V,
        size: PhysicalSize<u32>,
    ) -> Self {
        Self::create_internal(registry, root_item, size)
    }

    fn create_internal(
        registry: &mut TypeRegistry,
        element: impl Element,
        size: PhysicalSize<u32>,
    ) -> Self {
        let mut taffy = TaffyTree::default();
        let root = taffy
            .new_leaf(taffy::Style {
                size: taffy::Size {
                    // todo
                    width: length(size.width as f32),
                    height: length(size.height as f32),
                },
                ..Default::default()
            })
            .unwrap();

        let mut this = Self {
            taffy,
            widgets: HashMap::default(),
            views: HashMap::default(),
            root,
        };

        mount_children(registry, &mut this, root, element, None);

        this
    }

    pub(crate) fn insert(&mut self, widget: MountedWidget, parent: NodeId) -> NodeId {
        let id = self.taffy.new_leaf(widget.style().0).unwrap();
        self.taffy.add_child(parent, id).unwrap();

        self.widgets.insert(id, widget);

        id
    }

    pub(crate) fn insert_at(
        &mut self,
        element: MountedWidget,
        parent: NodeId,
        idx: usize,
    ) -> NodeId {
        let id = self.taffy.new_leaf(element.style().0).unwrap();

        self.taffy.insert_child_at_index(parent, idx, id).unwrap();
        self.widgets.insert(id, element);

        id
    }

    pub(crate) fn modify_if_necessary(&mut self, registry: &mut TypeRegistry, changed: NodeId) {
        self.comp_exchange(changed, registry);
    }

    fn comp_exchange(&mut self, view_id: NodeId, registry: &mut TypeRegistry) {
        debug_assert!(self.taffy.child_count(view_id) == 1);
        let only_child = self.taffy.child_at_index(view_id, 0).unwrap();

        let Some(view) = self.views.remove(&view_id) else {
            unreachable!()
        };

        view.0.dyn_cmp(only_child, self, registry);

        // todo avoid this by passing in tree?
        self.views.insert(view_id, view);
    }
}

#[doc(hidden)]
pub fn iter_elements_cmp<E: Element>(
    tree: &mut WidgetTree,
    processing: NodeId,
    new_element_at_position: E,
    registry: &mut TypeRegistry,
) {
    struct CompareInsertContext<'a> {
        tree: &'a mut WidgetTree,
        processing: NodeId,
        registry: &'a mut TypeRegistry,
        child_idx: usize,
    }

    impl<'a> RebuildContext for CompareInsertContext<'a> {
        fn rebuild_child<E: Element>(&mut self, e: E) {
            iter_elements_cmp(
                self.tree,
                self.tree
                    .taffy
                    .child_at_index(self.processing, self.child_idx)
                    .unwrap(),
                e,
                self.registry,
            );

            self.child_idx += 1;
        }
    }

    let element_at_current_position = tree.widgets.remove(&processing).unwrap();

    let BuildResult { widget, children } =
        new_element_at_position.compare_rebuild(element_at_current_position);

    tree.widgets.insert(processing, widget);

    if let Some(children) = children {
        let rebuilder = &mut CompareInsertContext {
            tree,
            processing,
            registry,
            child_idx: 0,
        };

        children.rebuild_children(rebuilder)
    }

    // self.processing

    // let ElementTree {
    //     taffy, elements, ..
    // } = tree;

    // let parent = taffy.parent(processing).unwrap();
    // let to_delete = iter_elements_from(&taffy, processing)
    //     .map(|it| it.1)
    //     .collect::<Vec<_>>();

    // for to_delete in to_delete {
    //     elements.remove(&to_delete).unwrap();
    //     taffy.remove(to_delete).unwrap();
    // }

    // let mut idx = 0;

    // while let false = taffy.child_at_index(parent, idx).unwrap() == processing {
    //     idx += 1;
    // }

    // taffy.remove(processing).unwrap();

    // mount_children(registry, tree, parent, with, Some(idx));

    // todo update style??
}

pub(crate) fn mount_children<T: Element>(
    registry: &mut TypeRegistry,
    tree: &mut WidgetTree,
    parent: NodeId,
    element: T,
    idx: Option<usize>,
) {
    struct Mounter<'a> {
        tree: &'a mut WidgetTree,
        parent: NodeId,
        registry: &'a mut TypeRegistry,
    }

    impl<'a> InsertContext for Mounter<'a> {
        fn insert_child<E: Element>(&mut self, e: E) {
            mount_children(&mut self.registry, self.tree, self.parent, e, None)
        }
    }

    let BuildResult { widget, children } = element.create(registry);

    if let Some(idx) = idx {
        tree.insert_at(widget, parent, idx);
    } else {
        tree.insert(widget, parent);
    }

    if let Some(children) = children {
        children.insert_children(&mut Mounter {
            tree,
            parent,
            registry,
        });
    }
}
