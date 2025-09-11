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

/// A css animation with all value components stored as raw CSS strings.
#[must_use]
#[derive(Debug, Clone)]
pub struct Animation {
    /// The animation-name
    name: String,
    /// The duration
    duration: String,
    /// The timing / easing function
    easing: String,
    /// The delay before it starts
    delay: String,
    /// The iteration count
    iteration_count: String,
    /// The direction
    direction: String,
    /// The fill-mode
    fill_mode: String,
    /// The play state
    state: String,
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

    /// Override / set the animation name.
    pub fn name(mut self, name: impl CssPropertyValue<Kind = KeyFrame>) -> Self {
        self.name = name.into_css();
        self
    }

    /// Set the time before the animation starts.
    pub fn delay(mut self, delay: impl CssPropertyValue<Kind = Duration>) -> Self {
        self.delay = delay.into_css();
        self
    }

    /// Set the duration.
    pub fn duration(mut self, duration: impl CssPropertyValue<Kind = Duration>) -> Self {
        self.duration = duration.into_css();
        self
    }

    /// Set the easing / timing function.
    pub fn easing(mut self, function: impl CssPropertyValue<Kind = EasingFunction>) -> Self {
        self.easing = function.into_css();
        self
    }

    /// Set the iteration count.
    pub fn iteration_count(
        mut self,
        count: impl CssPropertyValue<Kind = AnimationIterationCount>,
    ) -> Self {
        self.iteration_count = count.into_css();
        self
    }

    /// Convenience: make this animation repeat forever.
    pub fn infinite(mut self) -> Self {
        self.iteration_count = AnimationIterationCount::Infinite.into_css();
        self
    }

    /// Set the direction of the animation.
    pub fn direction(
        mut self,
        direction: impl CssPropertyValue<Kind = AnimationDirection>,
    ) -> Self {
        self.direction = direction.into_css();
        self
    }

    /// Set the animation fill-mode.
    pub fn fill_mode(mut self, mode: impl CssPropertyValue<Kind = AnimationFillMode>) -> Self {
        self.fill_mode = mode.into_css();
        self
    }

    /// Set whether the animation is running or not.
    pub fn state(mut self, state: impl CssPropertyValue<Kind = AnimationState>) -> Self {
        self.state = state.into_css();
        self
    }
}

impl IntoCss for Animation {
    fn into_css(self) -> String {
        // Order: duration | timing-function | delay | iteration-count | direction | fill-mode | play-state | name
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
