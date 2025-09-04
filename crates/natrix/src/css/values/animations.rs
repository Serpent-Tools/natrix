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

/// A css animation
#[must_use]
#[derive(Debug, Clone)]
#[cfg_attr(
    all(test, not(target_arch = "wasm32")),
    derive(proptest_derive::Arbitrary)
)]
pub struct Animation {
    /// The keyframe of this animation
    #[cfg_attr(
        all(test, not(target_arch = "wasm32")),
        proptest(value = r#"KeyFrame("slide")"#)
    )]
    pub name: KeyFrame,
    /// The duration of the animation
    pub duration: Duration,
    /// The easing function to use
    pub easing: EasingFunction,
    /// The delay before starting the animation
    pub delay: Duration,
    /// The amount of times the animation repeats, defaults to 1
    pub iteration_count: AnimationIterationCount,
    /// The direction the animation plays.
    pub direction: AnimationDirection,
    /// The fill mode for the animation
    pub fill_mode: AnimationFillMode,
    /// Is the animation paused or playing
    pub state: AnimationState,
}

impl KeyFrame {
    /// create a default `Animation` for this keyframe using the given duration
    pub fn animation(self, duration: Duration) -> Animation {
        Animation::new(self, duration)
    }
}

impl Animation {
    /// Create new animation for the given keyframe with default values.
    pub fn new(name: KeyFrame, duration: Duration) -> Self {
        Self {
            name,
            duration,
            easing: EasingFunction::default(),
            delay: Duration::ZERO,
            iteration_count: AnimationIterationCount::default(),
            direction: AnimationDirection::default(),
            fill_mode: AnimationFillMode::default(),
            state: AnimationState::default(),
        }
    }
    /// Set the time before the animation starts.
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Set the easing function
    pub fn easing(mut self, function: EasingFunction) -> Self {
        self.easing = function;
        self
    }

    /// Set the iteration count
    pub fn iteration_count(mut self, count: f32) -> Self {
        self.iteration_count = AnimationIterationCount::Finite(count);
        self
    }

    /// Make this animation repeat forever
    pub fn infinite(mut self) -> Self {
        self.iteration_count = AnimationIterationCount::Infinite;
        self
    }

    /// Set the direction of the animation
    pub fn direction(mut self, direction: AnimationDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Set the animation fill-mode
    pub fn fill_mode(mut self, mode: AnimationFillMode) -> Self {
        self.fill_mode = mode;
        self
    }

    /// Set whether the animation is running or not
    pub fn state(mut self, state: AnimationState) -> Self {
        self.state = state;
        self
    }
}

impl IntoCss for Animation {
    fn into_css(self) -> String {
        format!(
            "{} {} {} {} {} {} {} {}",
            self.duration.into_css(),
            self.easing.into_css(),
            self.delay.into_css(),
            self.iteration_count.into_css(),
            self.direction.into_css(),
            self.fill_mode.into_css(),
            self.state.into_css(),
            self.name.into_css(),
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
