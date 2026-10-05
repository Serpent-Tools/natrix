//! Apply and update html classes

use super::html_elements::MaybeDeferred;
use crate::reactivity::State;
use crate::reactivity::context::RenderCtx;
use crate::reactivity::dom_hooks::{ReactiveClass, SimpleReactive, SimpleReactiveResult};
use crate::web_value::{RawWebValue, WebValue};

/// The `WebValue` target for classes, see `HtmlElement::class`.
pub struct ClassName<C: State>(pub(crate) MaybeDeferred<C>);

impl<C: State> RawWebValue for ClassName<C> {
    type Arguments<'arg> = &'arg web_sys::Element;
}

impl<C: State> Default for ClassName<C> {
    fn default() -> Self {
        Self(MaybeDeferred::Static(None))
    }
}

impl<F, C, R> WebValue<ClassName<C>> for F
where
    F: Fn(RenderCtx<C>) -> R + 'static,
    R: WebValue<ClassName<C>> + 'static,
    C: State,
{
    type Kind = R::Kind;

    fn resolve(self, node: &web_sys::Element) -> ClassName<C> {
        let node = node.clone();
        ClassName(MaybeDeferred::Deferred(Box::new(
            move |ctx, rendering_state| {
                let hook = SimpleReactive::init_new(
                    Box::new(move |callback_ctx, callback_node| {
                        match self(callback_ctx).resolve(callback_node).0 {
                            MaybeDeferred::Static(value) => {
                                SimpleReactiveResult::Apply(ReactiveClass { data: value })
                            }
                            MaybeDeferred::Deferred(inner) => SimpleReactiveResult::Call(inner),
                        }
                    }),
                    node.clone(),
                    ctx,
                );
                rendering_state.hooks.push(hook);
            },
        )))
    }
}
