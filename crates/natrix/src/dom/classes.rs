//! Apply and update html classes

use std::borrow::Cow;

use super::html_elements::DeferredFunc;
use crate::reactivity::State;
use crate::reactivity::context::RenderCtx;
use crate::reactivity::dom_hooks::{ReactiveClass, SimpleReactive, SimpleReactiveResult};

/// The result of applying a class
pub(crate) enum ClassResult<C: State> {
    /// The class should be applied immedtialy
    SetIt(Option<Cow<'static, str>>),
    /// The class needs access to state
    Dynamic(DeferredFunc<C>),
}

/// A trait for converting a value to a class name
pub trait ToClass<C: State> {
    /// Convert the value to a class name
    fn calc_class(self, node: &web_sys::Element) -> ClassResult<C>;
}

impl<C: State, T: ToClass<C>> ToClass<C> for Option<T> {
    fn calc_class(self, node: &web_sys::Element) -> ClassResult<C> {
        if let Some(inner) = self {
            inner.calc_class(node)
        } else {
            ClassResult::SetIt(None)
        }
    }
}

impl<C: State, T: ToClass<C>, E: ToClass<C>> ToClass<C> for Result<T, E> {
    fn calc_class(self, node: &web_sys::Element) -> ClassResult<C> {
        match self {
            Ok(inner) => inner.calc_class(node),
            Err(inner) => inner.calc_class(node),
        }
    }
}

impl<F, C, R> ToClass<C> for F
where
    F: Fn(RenderCtx<C>) -> R + 'static,
    R: ToClass<C> + 'static,
    C: State,
{
    fn calc_class(self, node: &web_sys::Element) -> ClassResult<C> {
        let node = node.clone();

        ClassResult::Dynamic(Box::new(move |ctx, rendering_state| {
            let hook = SimpleReactive::init_new(
                Box::new(move |callback_ctx, callback_node| {
                    match self(callback_ctx).calc_class(callback_node) {
                        ClassResult::SetIt(value) => {
                            SimpleReactiveResult::Apply(ReactiveClass { data: value })
                        }
                        ClassResult::Dynamic(inner) => SimpleReactiveResult::Call(inner),
                    }
                }),
                node.clone(),
                ctx,
            );
            rendering_state.hooks.push(hook);
        }))
    }
}
