#![doc = include_str!(concat!("../", std::env!("CARGO_PKG_README")))]
#![cfg_attr(not(feature = "_internal_no_ssg"), forbid(unsafe_code))]

pub mod access;

pub mod async_utils;
pub mod css;
pub mod dom;
mod error_handling;
pub mod panics;
pub mod reactivity;
pub mod test_utils;
mod type_macros;
pub mod web_value;

pub use wasm_bindgen::intern;

thread_local! {
    /// A lazy initialized reference to the js document.
    static DOCUMENT: web_sys::Document = {
        #[expect(
            clippy::expect_used,
            reason = "A web framework cant do much without access to the document"
        )]
        web_sys::window()
            .expect("Window object not found")
            .document()
            .expect("Document object not found")
    };

    /// A lazy initialized reference to the js window.
    static WINDOW: web_sys::Window = {
        #[expect(
            clippy::expect_used,
            reason = "A web framework cant do much without access to the window"
        )]
        web_sys::window()
            .expect("Window object not found")
    };
}

/// Get the globally acquired document
///
/// This is cached so we dont need the slowdown of the js interop and `Result` handling for every
/// use of document.
pub(crate) fn get_document() -> web_sys::Document {
    DOCUMENT.with(Clone::clone)
}

/// Get the globally acquired window
///
/// This is cached so we dont need the slowdown of the js interop and `Result` handling for every
/// use of window.
pub(crate) fn get_window() -> web_sys::Window {
    WINDOW.with(Clone::clone)
}

/// Commonly used types and traits.
#[expect(clippy::min_ident_chars, reason = "intended api")]
pub mod prelude {
    pub use natrix_macros::State;

    pub use super::access::{Downgrade, Getter, Project, Ref, RefClosure};
    pub use super::css::property::Variable;
    pub use super::css::selectors::{
        Class,
        Id,
        IntoComplexSelector,
        IntoCompoundSelector,
        IntoFinalizedSelector,
    };
    pub use super::dom::{Element, events, html_elements as e};
    pub use super::reactivity::State;
    pub use super::reactivity::context::{EventCtx, RenderCtx};
    pub use super::reactivity::signal::Signal;
    pub use super::{field, with};
}

pub use dom::Element;
pub use natrix_macros::{State, asset, format_elements};
pub use reactivity::context::{EventCtx, RenderCtx};
pub use reactivity::mount::mount;

/// Public exports of internal data structures for `natrix_macros` (and `macro_rules`) to use in generated code.
#[doc(hidden)]
pub mod macro_ref {
    pub use const_base;
    pub use const_sha1;
    #[cfg(feature = "_internal_collect_css")]
    pub use inventory;
    pub use log;

    pub use super::css;
    pub use super::dom::element::Element;
    pub use super::reactivity::State;
}
