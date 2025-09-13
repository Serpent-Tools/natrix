//! Implementation of css animations

use std::time::Duration;

use crate::css::IntoCss;
use crate::css::keyframes::KeyFrame;
use crate::css::values::CssPropertyValue;
pub use crate::css::values::{
    AnimationDirection,
    AnimationFillMode,
    AnimationIterationCount,
    AnimationState,
    EasingFunction,
};

super::define_css_shorthand! {
    /// A css animation
    pub struct Animation {
        /// <https://developer.mozilla.org/en-US/docs/Web/CSS/animation-name>
        name: String,
        {
            /// <https://developer.mozilla.org/en-US/docs/Web/CSS/animation-duration>
            duration: Duration,
            /// <https://developer.mozilla.org/en-US/docs/Web/CSS/animation-easing>
            easing: EasingFunction,
            /// <https://developer.mozilla.org/en-US/docs/Web/CSS/animation-delay>
            delay: Duration,
            /// <https://developer.mozilla.org/en-US/docs/Web/CSS/animation-iteration-count>
            iteration_count: AnimationIterationCount,
            /// <https://developer.mozilla.org/en-US/docs/Web/CSS/animation-direction>
            direction: AnimationDirection,
            /// <https://developer.mozilla.org/en-US/docs/Web/CSS/animation-fill-mode>
            fill_mode: AnimationFillMode,
            /// <https://developer.mozilla.org/en-US/docs/Web/CSS/animation-state>
            state: AnimationState,
        }
    }
}

impl KeyFrame {
    /// Create a default `Animation` for this keyframe using the given duration.
    pub fn animation(self, duration: impl CssPropertyValue<Kind = Duration>) -> Animation {
        Animation::new(self, duration)
    }
}

impl Animation {
    /// Create new animation for the given keyframe with default values.
    pub fn new(
        name: impl CssPropertyValue<Kind = KeyFrame>,
        duration: impl CssPropertyValue<Kind = Duration>,
    ) -> Self {
        Self {
            name: name.into_css(),
            duration: duration.into_css(),
            easing: EasingFunction::default().into_css(),
            delay: Duration::ZERO.into_css(),
            iteration_count: AnimationIterationCount::default().into_css(),
            direction: AnimationDirection::default().into_css(),
            fill_mode: AnimationFillMode::default().into_css(),
            state: AnimationState::default().into_css(),
        }
    }
}

impl IntoCss for Animation {
    fn into_css(self) -> String {
        format!(
            "{} {} {} {} {} {} {} {}",
            self.duration,
            self.easing,
            self.delay,
            self.iteration_count,
            self.direction,
            self.fill_mode,
            self.state,
            self.name,
        )
    }
}

impl IntoCss for Vec<Animation> {
    fn into_css(self) -> String {
        self.into_iter()
            .map(IntoCss::into_css)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl CssPropertyValue for Animation {
    type Kind = Animation;
}
impl CssPropertyValue for Vec<Animation> {
    type Kind = Animation;
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod arbitrary_impl {
    use proptest::prelude::*;

    use super::*;

    impl Arbitrary for Animation {
        type Parameters = ();
        type Strategy = BoxedStrategy<Self>;

        fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
            (
                any::<Duration>(), // duration
                any::<EasingFunction>(),
                any::<Duration>(), // delay
                any::<AnimationIterationCount>(),
                any::<AnimationDirection>(),
                any::<AnimationFillMode>(),
                any::<AnimationState>(),
            )
                .prop_map(
                    |(duration, easing, delay, iteration_count, direction, fill_mode, state)| {
                        Animation::new(KeyFrame("slide"), duration)
                            .easing(easing)
                            .delay(delay)
                            .iteration_count(iteration_count)
                            .direction(direction)
                            .fill_mode(fill_mode)
                            .state(state)
                    },
                )
                .boxed()
        }
    }
}
