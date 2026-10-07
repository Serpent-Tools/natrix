//! DOM-specific reactive hook implementations.
//!
//! This module contains concrete implementations of the `ReactiveHook` trait
//! for updating DOM elements in response to signal changes:
//! - `ReactiveNode`: Swaps out entire DOM nodes
//! - `SimpleReactive`: Generic wrapper for simple reactive operations (attributes, classes, etc.)

use std::borrow::Cow;

use wasm_bindgen::JsCast;

use crate::dom::element::{ElementRenderResult, MaybeStaticElement, generate_fallback_node};
use crate::dom::html_elements::DeferredFunc;
use crate::error_handling::{log_or_panic, log_or_panic_result};
use crate::get_document;
use crate::reactivity::context::{InnerCtx, RenderCtx};
use crate::reactivity::core::{HookKey, ReactiveHook, RenderingState, UpdateResult};
use crate::reactivity::{KeepAlive, State};

/// A reactive closure that returns a element
type ElementClosure<S> = Box<dyn Fn(RenderCtx<'_, '_, S>) -> MaybeStaticElement<S> + 'static>;

/// Reactive hook for swapping out an entire DOM node.
pub(crate) struct ReactiveNode<S: State> {
    /// The callback to produce nodes
    callback: ElementClosure<S>,
    /// The current rendered node to replace
    target_node: web_sys::Node,
    /// Vector of various objects to be kept alive for the duration of the rendered content
    keep_alive: Vec<KeepAlive>,
    /// Hooks that are a child of this
    hooks: Vec<HookKey>,
}

impl<S: State> ReactiveNode<S> {
    /// Render this hook and simply return the node
    ///
    /// INVARIANT: This function works with the assumption what it returns will be put in its
    /// `target_node` field. This function is split out to facilitate `Self::create_initial`
    fn render(&mut self, ctx: &mut InnerCtx<S>, you: HookKey) -> ElementRenderResult {
        let element = ctx.track_reads(you, |ctx| {
            (self.callback)(RenderCtx {
                ctx,
                render_state: RenderingState {
                    keep_alive: &mut self.keep_alive,
                    hooks: &mut self.hooks,
                },
            })
        });

        let mut state = RenderingState {
            keep_alive: &mut self.keep_alive,
            hooks: &mut self.hooks,
        };
        element.render_static(ctx, &mut state)
    }

    /// Create a new `ReactiveNode` registering the initial dependencies and returning both the
    /// `HookKey` for it and the initial node (Which should be inserted in the dom)
    pub(crate) fn create_initial(
        callback: ElementClosure<S>,
        ctx: &mut InnerCtx<S>,
    ) -> (HookKey, web_sys::Node) {
        let me = ctx.hooks.reserve_key();

        let Some(dummy_node) = get_document().body() else {
            log_or_panic!("Document body not found");
            return (me, generate_fallback_node());
        };
        let dummy_node = dummy_node.into();

        let mut this = Self {
            callback,
            target_node: dummy_node,
            keep_alive: Vec::new(),
            hooks: Vec::new(),
        };
        let node = this.render(ctx, me).into_node();
        this.target_node = node.clone();
        ctx.hooks.set_hook(me, Box::new(this));

        (me, node)
    }
}

impl<S: State> ReactiveHook<S> for ReactiveNode<S> {
    fn update(&mut self, ctx: &mut InnerCtx<S>, you: HookKey) -> UpdateResult {
        let this = &mut *self;
        let hooks = std::mem::take(&mut this.hooks);
        let new_node = this.render(ctx, you);

        let new_node = match new_node {
            ElementRenderResult::Node(new_node) => new_node,
            ElementRenderResult::Text(new_text) => {
                if let Some(target_node) = this.target_node.dyn_ref::<web_sys::Text>() {
                    target_node.set_text_content(Some(&new_text));
                    return UpdateResult::DropHooks(hooks);
                }

                get_document().create_text_node(&new_text).into()
            }
        };

        let Some(parent) = this.target_node.parent_node() else {
            log_or_panic!("Parent node of target node not found.");
            return UpdateResult::DropHooks(hooks);
        };

        log_or_panic_result!(
            parent.replace_child(&new_node, &this.target_node),
            "Failed to replace parent"
        );
        this.target_node = new_node;

        UpdateResult::DropHooks(hooks)
    }

    fn drop_us(self: Box<Self>) -> Vec<HookKey> {
        self.hooks
    }
}

/// A trait to allow `SimpleReactive` to deduplicate common reactive logic for attributes, classes,
/// styles, etc
pub(crate) trait ReactiveValue {
    /// Any potential state needed to apply the change
    type State: Default;

    /// Actually apply the change
    fn apply(self, node: &web_sys::Element, state: &mut Self::State);
}

/// The result of a simple reactive call
pub(crate) enum SimpleReactiveResult<S: State, K> {
    /// Apply the value
    Apply(K),
    /// Call the inner reactive function
    Call(DeferredFunc<S>),
}

/// The callback used by simple reactive
type SimpleReactiveCallback<S, K> =
    Box<dyn Fn(RenderCtx<'_, '_, S>, &web_sys::Element) -> SimpleReactiveResult<S, K> + 'static>;

/// A common wrapper for simple reactive operations to deduplicate dependency tracking code
pub(crate) struct SimpleReactive<S: State, K: ReactiveValue> {
    /// The callback to call, takes state and returns the needed data for the reactive
    /// transformation
    callback: SimpleReactiveCallback<S, K>,
    /// The node to apply transformations to
    node: web_sys::Element,
    /// Vector of various objects to be kept alive for the duration of the rendered content
    keep_alive: Vec<KeepAlive>,
    /// Hooks to use
    hooks: Vec<HookKey>,
    /// The state needed to apply the transformation
    state: K::State,
}

impl<S: State, K: ReactiveValue> ReactiveHook<S> for SimpleReactive<S, K> {
    fn drop_us(self: Box<Self>) -> Vec<HookKey> {
        self.hooks
    }

    fn update(&mut self, ctx: &mut InnerCtx<S>, you: HookKey) -> UpdateResult {
        let hooks = std::mem::take(&mut self.hooks);

        self.keep_alive.clear();

        let value = ctx.track_reads(you, |ctx| {
            (self.callback)(
                RenderCtx {
                    ctx,
                    render_state: RenderingState {
                        keep_alive: &mut self.keep_alive,
                        hooks: &mut self.hooks,
                    },
                },
                &self.node,
            )
        });

        match value {
            SimpleReactiveResult::Apply(value) => {
                value.apply(&self.node, &mut self.state);
            }
            SimpleReactiveResult::Call(func) => func(
                ctx,
                &mut RenderingState {
                    keep_alive: &mut self.keep_alive,
                    hooks: &mut self.hooks,
                },
            ),
        }

        UpdateResult::DropHooks(hooks)
    }
}

impl<S: State, K: ReactiveValue + 'static> SimpleReactive<S, K> {
    /// Creates a new simple reactive hook, applying the initial transformation.
    /// Returns a hookkey of the hook
    pub(crate) fn init_new(
        callback: SimpleReactiveCallback<S, K>,
        node: web_sys::Element,
        ctx: &mut InnerCtx<S>,
    ) -> HookKey {
        let me = ctx.hooks.reserve_key();

        let mut this = Self {
            callback,
            node,
            keep_alive: Vec::new(),
            hooks: Vec::new(),
            state: K::State::default(),
        };
        this.update(ctx, me);

        ctx.hooks.set_hook(me, Box::new(this));

        me
    }
}

/// Reactively set an element attribute
pub(crate) struct ReactiveAttribute {
    /// The attribute name to set
    pub(crate) name: &'static str,
    /// The attribute value to apply
    pub(crate) data: Option<Cow<'static, str>>,
}

impl ReactiveValue for ReactiveAttribute {
    type State = ();

    fn apply(self, node: &web_sys::Element, _state: &mut Self::State) {
        if let Some(res) = self.data {
            log_or_panic_result!(
                node.set_attribute(self.name, &res),
                "Failed to update attribute"
            );
        } else {
            log_or_panic_result!(
                node.remove_attribute(self.name),
                "Failed to remove attribute"
            );
        }
    }
}

/// Reactively set a element class
pub(crate) struct ReactiveClass {
    /// The class value to apply
    pub(crate) data: Option<Cow<'static, str>>,
}

impl ReactiveValue for ReactiveClass {
    type State = Option<Cow<'static, str>>;

    fn apply(self, node: &web_sys::Element, state: &mut Self::State) {
        let class_list = node.class_list();

        match (&state, &self.data) {
            (None, None) => {}
            (Some(prev), None) => {
                log_or_panic_result!(class_list.remove_1(prev), "Failed to remove class");
            }
            (None, Some(new)) => {
                log_or_panic_result!(class_list.add_1(new), "Failed to add class");
            }
            (Some(prev), Some(new)) => {
                log_or_panic_result!(class_list.replace(prev, new), "Failed to replace class");
            }
        }
        *state = self.data;
    }
}
