//! Implement css keyframes

use super::values::IntoCss;
use crate::css::prelude::RuleBody;
use crate::css::values::CssPropertyValue;
use crate::css::values::units::Percentage;

/// The name of a keyframe
#[derive(Clone, Copy, Debug)]
pub struct KeyFrame(pub &'static str);

impl IntoCss for KeyFrame {
    fn into_css(self) -> String {
        super::as_css_identifier(self.0)
    }
}

impl CssPropertyValue for KeyFrame {
    type Kind = KeyFrame;
}

/// A keyframe definition
#[derive(Default, Clone)]
#[must_use]
pub struct KeyframeDefinition(Vec<(Percentage, RuleBody)>);

impl KeyframeDefinition {
    /// Create a new empty keyframe definition
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert this into css
    #[doc(hidden)]
    #[must_use]
    pub fn to_css(self, name: &KeyFrame) -> String {
        let mut inner = String::new();
        for (frame, body) in self.0 {
            let rule = format!("{}, {{{}}}", frame.into_css(), body.into_css());
            inner.push_str(&rule);
        }

        format!(
            "@keyframes {} {{{inner}}}",
            super::as_css_identifier(name.0)
        )
    }

    /// Add a selector
    pub fn frame(mut self, percentage: Percentage, body: RuleBody) -> Self {
        self.0.push((percentage, body));
        self
    }
}

/// Define and register a `@keyframe` to go in the bundler.
///
/// Any code in here wont be included in the final wasm build.
/// And will be run at compile time.
/// This macro must not be called from within a function.
#[macro_export]
macro_rules! register_keyframe {
    ($vis:vis const $name:ident = $def:expr) => {
        $vis const $name: $crate::macro_ref::css::keyframes::KeyFrame = $crate::macro_ref::css::keyframes::KeyFrame($crate::unique_str!());
        $crate::register_raw_css!({
            use $crate::macro_ref::css::prelude::*;
            use $crate::macro_ref::css::keyframes::KeyframeDefinition;
            let result: $crate::macro_ref::css::keyframes::KeyframeDefinition = $def;
            result.to_css(&$name)
        });
    };
}
