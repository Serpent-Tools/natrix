//! Implementation of the `Element` trait for various abstract types.

use std::borrow::Cow;

use super::HtmlElement;
use crate::error_handling::log_or_panic;
use crate::reactivity::State;
use crate::reactivity::context::{InnerCtx, RenderCtx};
use crate::reactivity::core::RenderingState;
use crate::reactivity::dom_hooks::ReactiveNode;
use crate::type_macros;

/// A result of the rendering process.
pub(crate) enum ElementRenderResult {
    /// A generic node.
    Node(web_sys::Node),
    /// A text node.
    Text(Cow<'static, str>),
}

impl ElementRenderResult {
    /// Convert to a `web_sys::Node`.
    pub(crate) fn into_node(self) -> web_sys::Node {
        match self {
            ElementRenderResult::Node(node) => node,
            ElementRenderResult::Text(text) => {
                if let Ok(node) = web_sys::Text::new_with_data(&text) {
                    node.into()
                } else {
                    log_or_panic!("Failed to create text node");
                    generate_fallback_node()
                }
            }
        }
    }
}

/// The result of a `.render` call.
pub enum MaybeStaticElement<C: State> {
    /// A already statically rendered element.
    Static(ElementRenderResult),
    /// A html element
    Html(HtmlElement<C>),
    /// A element that needs access to state to be rendered.
    Dynamic(Box<dyn DynElement<C>>),
}

impl<C: State> MaybeStaticElement<C> {
    /// Convert the element into a `web_sys::Node`.
    // TODO: Refactor this
    pub fn render_static(
        self,
        ctx: &mut InnerCtx<C>,
        render_state: &mut RenderingState,
    ) -> ElementRenderResult {
        match self {
            MaybeStaticElement::Static(element) => element,
            MaybeStaticElement::Html(mut html) => {
                for modification in html.drain_deferred() {
                    modification(ctx, render_state);
                }

                ElementRenderResult::Node(html.get_element().clone().into())
            }
            MaybeStaticElement::Dynamic(element) => element.render(ctx, render_state),
        }
    }
}

/// A element is anything that can be rendered in the dom.
/// This is ofc `HtmlElement`, but also strings, numerics, and even closures.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid Element.",
    note = "If this is a reference/signal you might have forgotten to dereference."
)]
pub trait Element<C: State>: 'static {
    /// Convert the element into a `MaybeStaticElement`.
    fn render(self) -> MaybeStaticElement<C>;
}

/// A dynamic element
pub(crate) trait DynElement<C: State> {
    /// Render the element.
    fn render(
        self: Box<Self>,
        ctx: &mut InnerCtx<C>,
        render_state: &mut RenderingState,
    ) -> ElementRenderResult;
}

impl<C: State> Element<C> for web_sys::Node {
    #[inline]
    fn render(self) -> MaybeStaticElement<C> {
        MaybeStaticElement::Static(ElementRenderResult::Node(self))
    }
}

impl<C: State, T: Element<C>> Element<C> for Option<T> {
    #[inline]
    fn render(self) -> MaybeStaticElement<C> {
        match self {
            Some(element) => element.render(),
            None => generate_fallback_node().render(),
        }
    }
}

impl<C: State, T: Element<C>, E: Element<C>> Element<C> for Result<T, E> {
    #[inline]
    fn render(self) -> MaybeStaticElement<C> {
        match self {
            Ok(element) => element.render(),
            Err(element) => element.render(),
        }
    }
}

/// Generate a Element implementation for a type that can be converted to `&str`
macro_rules! string_element {
    ($t:ty, $cow:expr) => {
        impl<C: State> Element<C> for $t {
            #[inline]
            fn render(self) -> MaybeStaticElement<C> {
                MaybeStaticElement::Static(ElementRenderResult::Text(($cow)(self)))
            }
        }
    };
}
type_macros::strings!(string_element);

/// Generate a implementation of `Element` for a specific numeric type.
macro_rules! numeric_element {
    ($T:ident, $fmt:ident, $_name:ident) => {
        impl<C: State> Element<C> for $T {
            #[inline]
            fn render(self) -> MaybeStaticElement<C> {
                let mut buffer = $fmt::Buffer::new();
                let result = buffer.format(self);

                MaybeStaticElement::Static(ElementRenderResult::Text(Cow::Owned(result.to_owned())))
            }
        }
    };
}
type_macros::numerics!(numeric_element);

/// Attempt to create a comment node.
/// If this fails (wrongly) convert the error to a comment node.
/// This allows us to satisfy a non-Result `web_sys::Node` return type.
/// This conversion should never happen, but if it does, code down the line will simply hit a error
/// and will ignore it as needed.
pub(crate) fn generate_fallback_node() -> web_sys::Node {
    web_sys::Comment::new()
        .unwrap_or_else(wasm_bindgen::JsCast::unchecked_into)
        .into()
}

impl<C: State> Element<C> for MaybeStaticElement<C> {
    #[inline]
    fn render(self) -> MaybeStaticElement<C> {
        self
    }
}

impl<F, C, R> DynElement<C> for F
where
    F: Fn(RenderCtx<C>) -> R + 'static,
    R: Element<C> + 'static,
    C: State,
{
    fn render(
        self: Box<Self>,
        ctx: &mut InnerCtx<C>,
        render_state: &mut RenderingState,
    ) -> ElementRenderResult {
        let this = *self;
        let (me, node) = ReactiveNode::create_initial(
            Box::new(move |handler_ctx| this(handler_ctx).render()),
            ctx,
        );
        render_state.hooks.push(me);
        ElementRenderResult::Node(node)
    }
}

impl<F, C, R> Element<C> for F
where
    F: Fn(RenderCtx<C>) -> R + 'static,
    R: Element<C> + 'static,
    C: State,
{
    #[inline]
    fn render(self) -> MaybeStaticElement<C> {
        MaybeStaticElement::Dynamic(Box::new(self))
    }
}
