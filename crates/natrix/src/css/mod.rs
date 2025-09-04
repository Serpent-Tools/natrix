//! Create css styles

// NOTE:
// This module does not generally need to be efficient, it just needs to be ergonomic.
// The code in this module is intended to be used at bundling time, and hence will not be included
// in production applications.

use std::fmt::Write;

use crate::error_handling::log_or_panic_result;

pub mod keyframes;
pub mod property;
pub mod selectors;
pub mod values;

/// Css prelude
/// This is auto star imported in the various `register_*` macros
pub mod prelude {
    pub use super::property::RuleBody;
    pub use super::{IntoCss, property, selectors, values};
    pub use crate::{const_unwrap, selector_list};
}

/// Convert a value to a css
pub trait IntoCss {
    /// Convert a value to a css value string
    fn into_css(self) -> String;
}

/// Escape special characthers in string such that it becomes a valid css identifier
#[must_use]
pub fn as_css_identifier(input: &str) -> String {
    let mut result = String::with_capacity(input.len().saturating_mul(4));
    for c in input.chars() {
        let res = write!(&mut result, "\\{:x} ", c as u32);
        log_or_panic_result!(res, "Failed to write to string (???).");
    }
    result
}

/// Struct to let `inventory` collect css from all across the dep graph
#[doc(hidden)]
#[cfg(feature = "_internal_collect_css")]
pub struct CssEmit(pub fn() -> String);

#[cfg(feature = "_internal_collect_css")]
inventory::collect!(CssEmit);

#[cfg(all(feature = "_internal_no_ssg", target_arch = "wasm32"))]
#[expect(
    unsafe_code,
    reason = "This is required for inventory to work, it is not included in production builds"
)]
unsafe extern "C" {
    fn __wasm_call_ctors();
}

/// Register a css stylesheet to go in the bundler.
/// Generally you should prefer the various other `register_` helpers.
///
/// Any code in here wont be included in the final wasm build.
/// And will be run at compile time.
/// This macro must not be called from within a function.
#[macro_export]
#[cfg(feature = "_internal_collect_css")]
macro_rules! register_raw_css {
    ($style:expr) => {
        $crate::macro_ref::inventory::submit!($crate::macro_ref::css::CssEmit(|| {
            $crate::macro_ref::log::trace!(concat!("generating css for ", file!(), " ", line!()));
            $style
        }));
    };
}

/// Register a css string to go in the bundler.
/// Generally you should prefer the various other `register_` helpers.
///
/// Any code in here wont be included in the final wasm build.
/// And will be run at compile time.
/// This macro must not be called from within a function.
#[macro_export]
#[cfg(not(feature = "_internal_collect_css"))]
macro_rules! register_raw_css {
    ($style:expr) => {
        const _: fn() -> String = || {
            $crate::macro_ref::log::warn!("Register css code called in non-collection mode");
            $style
        };
    };
}

/// Register a `RuleCollection` to go in the bundler.
///
/// Any code in here wont be included in the final wasm build.
/// And will be run at compile time.
/// This macro must not be called from within a function.
#[macro_export]
macro_rules! register_rules {
    ($collection:expr) => {
        $crate::register_raw_css!({
            use $crate::macro_ref::css::prelude::*;
            let result: $crate::macro_ref::css::property::RuleCollection = $collection;
            result.into_css()
        });
    };
}

/// Register a css rule to go in the bundler.
///
/// Any code in here wont be included in the final wasm build.
/// And will be run at compile time.
/// This macro must not be called from within a function.
#[macro_export]
macro_rules! register_rule {
    // NOTE: This is here to make IDE support nicer for partial macro invocations.
    ($selectors:expr $(,)?) => {
        $crate::register_rules!(
            $crate::macro_ref::css::property::RuleCollection::new().rule(
                $selectors,
                $crate::macro_ref::css::property::RuleBody::new()
            )
        );
        compile_error!("Missing rule body");
    };
    ($selectors:expr, $body:expr) => {
        $crate::register_rules!(
            $crate::macro_ref::css::property::RuleCollection::new().rule($selectors, $body)
        );
    };
}

/// Do the css collection and either emit to STDOUT or inject into dom
/// Depending on the selected feature flags.
#[cfg(feature = "_internal_collect_css")]
pub(crate) fn do_css_setup() {
    let result = collect_css();

    log::trace!("Produced css: {result}");

    #[cfg(feature = "_internal_no_ssg")]
    css_runtime(&result);

    #[cfg(feature = "_internal_bundle")]
    css_emit(&result);
}

/// Collect the css into a string.
#[cfg(feature = "_internal_collect_css")]
fn collect_css() -> String {
    log::info!("Collecting css");

    #[cfg(all(feature = "_internal_no_ssg", target_arch = "wasm32"))]
    #[expect(
        unsafe_code,
        reason = "This is required for inventory to work on wasm, it is not included in production builds"
    )]
    unsafe {
        log::trace!("Calling ctors");
        __wasm_call_ctors();
    }

    log::trace!("Collecting strings");
    let mut result = String::new();
    for emit in inventory::iter::<CssEmit> {
        result.push_str(&(emit.0)());
    }
    result
}

/// Inject the css into the dom at runtime
#[cfg(feature = "_internal_no_ssg")]
#[expect(
    clippy::expect_used,
    reason = "This happens early, and is meant for dev mode only"
)]
fn css_runtime(css_string: &str) {
    log::debug!("Injecting css into document");
    let document = crate::get_document();
    let style = document
        .create_element("style")
        .expect("Failed to create style element");

    style.set_inner_html(css_string);

    let body = document.body().expect("No body found");
    body.append_child(&style)
        .expect("Failed to append style element");
}

/// Print out the css to provide it to the bundler
///
/// # Design
/// Why not just `println!` in `do_css_collect`? because we might refactor this to use something a
/// bit more structured later (json, files, whatever)
#[cfg(feature = "_internal_bundle")]
fn css_emit(css_string: &str) {
    log::debug!("Emitting css to bundler");
    println!("{css_string}");
}

/// Create a unique string
///
/// This is a hash of the filename + line number + column (computed at compile time)
///
/// This is used internally by the `class` and `id` macros
/// ```rust
/// # use natrix::prelude::*;
/// use natrix::class;
///
/// const MY_CLASS: Class = class!(); // <-- uses `unique_str`
/// ```
#[macro_export]
macro_rules! unique_str {
    () => {{
        const RAW: &str = concat!(file!(), "-", line!(), "-", column!());
        const HASHED: [u8; 20] = $crate::macro_ref::const_sha1::sha1(RAW.as_bytes()).as_bytes();
        const ENCODED: &str = $crate::macro_ref::const_base::encode_as_str!(
            &HASHED,
            $crate::macro_ref::const_base::Config::B64_URL_SAFE.end_padding(false),
        );

        ENCODED
    }};
}

/// Check if a string is valid css
#[cfg(all(test, not(target_arch = "wasm32")))]
#[expect(clippy::panic, clippy::expect_used, reason = "This is meant for tests")]
fn assert_valid_css(string: &str) {
    let warnings = std::sync::Arc::default();
    let result = lightningcss::stylesheet::StyleSheet::parse(
        string,
        lightningcss::stylesheet::ParserOptions {
            warnings: Some(std::sync::Arc::clone(&warnings)),
            error_recovery: false,
            ..Default::default()
        },
    );

    match result {
        Err(error) => {
            panic!("The following code was not valid css\n{string}\nerror: {error}");
        }

        Ok(stylesheet) => {
            let warnings = warnings.read().expect("Failed to get lock");
            if !warnings.is_empty() {
                for warning in warnings.iter() {
                    eprintln!("{warning}");
                }
                panic!("The following code produced warnings\n{string}");
            }

            let debug_reps = format!("{stylesheet:?}");
            let invalid = debug_reps.contains("Unparsed") || debug_reps.contains("Unknown");
            assert!(
                !invalid,
                "Found indications of invalid css\n{string}\n{stylesheet:?}"
            );
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    #[test]
    fn unique_is_unique() {
        assert_ne!(unique_str!(), unique_str!());
    }

    #[cfg(feature = "_internal_collect_css")]
    mod test_collection {
        use crate::prelude::*;

        const BUTTON_CLASS: crate::prelude::Class = crate::class!();

        crate::register_rule!(
            BUTTON_CLASS,
            RuleBody::new().align_content(values::ContentPosition::Start)
        );
        crate::register_rules!(
            property::RuleCollection::new()
                .rule(
                    BUTTON_CLASS.child(BUTTON_CLASS),
                    RuleBody::new().align_content(values::ContentPosition::FlexStart)
                )
                .rule(
                    BUTTON_CLASS.next_sibling(BUTTON_CLASS),
                    RuleBody::new().align_content(values::BaselinePosition::First)
                )
        );
        crate::register_keyframe!(
            const SLIDE = KeyframeDefinition::new()
            .frame(values::Percentage(0.0), RuleBody::new().align_content(values::Normal))
            .frame(values::Percentage(100.0), RuleBody::new().align_content(values::Normal))
        );

        #[test]
        fn test_collect() {
            let result = super::super::collect_css();
            super::super::assert_valid_css(&result);
        }
    }

    proptest::proptest! {
        #[test]
        fn test_as_css_identifier(input in ".+") {
            let result = super::as_css_identifier(&input);
            let test_body = format!("#{result}{{}}");
            super::assert_valid_css(&test_body);
        }
    }
}
