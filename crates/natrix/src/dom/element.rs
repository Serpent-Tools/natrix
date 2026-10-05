//! Implementation of the `Element` trait for various abstract types.

use std::borrow::Cow;

use super::HtmlElement;
use crate::error_handling::log_or_panic;
use crate::reactivity::State;
use crate::reactivity::context::{InnerCtx, RenderCtx};
use crate::reactivity::core::RenderingState;
use crate::reactivity::dom_hooks::ReactiveNode;
use crate::web_value::{RawWebValue, WebValue};

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

/// A element that is either already rendered, or needs further processing to be inserted.
pub(crate) enum MaybeStaticElement<C: State> {
    /// A already statically rendered element.
    Static(ElementRenderResult),
    /// A html element
    Html(HtmlElement<C>),
    /// A element that needs access to state to be rendered.
    Dynamic(Box<dyn DynElement<C>>),
}

/// The `WebValue` target for elements, see `Element`.
pub struct Node<C: State>(pub(crate) MaybeStaticElement<C>);

impl<C: State> From<Cow<'static, str>> for Node<C> {
    fn from(value: Cow<'static, str>) -> Self {
        Self(MaybeStaticElement::Static(ElementRenderResult::Text(value)))
    }
}

impl<C: State> RawWebValue for Node<C> {
    type Arguments<'arg> = ();
}

impl<C: State> Default for Node<C> {
    fn default() -> Self {
        Self(MaybeStaticElement::Static(ElementRenderResult::Node(
            generate_fallback_node(),
        )))
    }
}

impl<C: State> MaybeStaticElement<C> {
    /// Convert the element into a `web_sys::Node`.
    pub(crate) fn render_static(
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
///
/// NOTE: You should not implement this trait, instead create functions/methods on your types that
/// return `impl Element`, if you really want to implement this trait implement
/// `WebValue<Node<C>>` instead.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid Element.",
    note = "If this is a reference/signal you might have forgotten to dereference."
)]
pub trait Element<C: State>: WebValue<Node<C>> {
    /// Alias for `WebValue::resolve` called with the empty tuple elements use.
    ///
    /// i.e `foo.resolve(())` and `foo.render()` are equivalent.
    fn render(self) -> Node<C>
    where
        Self: Sized,
    {
        self.resolve(())
    }
}

impl<C, T> Element<C> for T
where
    T: WebValue<Node<C>>,
    C: State,
{
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

impl<C: State> WebValue<Node<C>> for web_sys::Node {
    type Kind = Node<C>;

    #[inline]
    fn resolve(self, _arguments: ()) -> Node<C> {
        Node(MaybeStaticElement::Static(ElementRenderResult::Node(self)))
    }
}

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

impl<F, C, R> DynElement<C> for F
where
    F: Fn(RenderCtx<C>) -> R + 'static,
    R: WebValue<Node<C>> + 'static,
    C: State,
{
    fn render(
        self: Box<Self>,
        ctx: &mut InnerCtx<C>,
        render_state: &mut RenderingState,
    ) -> ElementRenderResult {
        let this = *self;
        let (me, node) = ReactiveNode::create_initial(
            Box::new(move |handler_ctx| this(handler_ctx).resolve(()).0),
            ctx,
        );
        render_state.hooks.push(me);
        ElementRenderResult::Node(node)
    }
}

impl<F, C, R> WebValue<Node<C>> for F
where
    F: Fn(RenderCtx<C>) -> R + 'static,
    R: WebValue<Node<C>> + 'static,
    C: State,
{
    type Kind = R::Kind;

    #[inline]
    fn resolve(self, _arguments: ()) -> Node<C> {
        Node(MaybeStaticElement::Dynamic(Box::new(self)))
    }
}
