//! Types for the various css values

mod animations;
mod colors;
pub mod units;

use std::fmt::Write;
use std::marker::PhantomData;
use std::time::Duration;

pub use animations::*;
pub use colors::Color;

pub use super::IntoCss;
use crate::error_handling::{log_or_panic, log_or_panic_result};
use crate::type_macros;

/// A css value thats valid in a property
pub trait CssPropertyValue: IntoCss {
    /// The kind of value, this is used to enable some stuff like css variables.
    /// But also allow us to easialy do things like declare a property supports numeics.
    type Kind;
}

impl IntoCss for Duration {
    fn into_css(self) -> String {
        format!("{}ms", self.as_secs_f64())
    }
}

/// The type used in `CssPropertyValue` to signal a numeric, such as u8, i16, f32, etc.
pub struct KindNumeric;

/// generate css traits for a numeric
macro_rules! impl_numerics {
    ($t:ident, $fmt:ident, $name:ident) => {
        impl IntoCss for $t {
            #[inline]
            fn into_css(self) -> String {
                let mut buffer = $fmt::Buffer::new();
                let result = buffer.format(self);
                result.to_string()
            }
        }
        impl CssPropertyValue for $t {
            type Kind = KindNumeric;
        }
    };
}
type_macros::numerics!(impl_numerics);

impl<A: IntoCss, B: IntoCss> IntoCss for (A, B) {
    fn into_css(self) -> String {
        format!("{} {}", self.0.into_css(), self.1.into_css())
    }
}
impl<A: CssPropertyValue, B: CssPropertyValue> CssPropertyValue for (A, B) {
    type Kind = (A::Kind, B::Kind);
}

/// Define a `ToCssValue` enum
macro_rules! define_enum {
    (
        $(#[$enum_meta:meta])*
        enum $name:ident,
        $value_name:literal,
        $mdn_url:literal,
        {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident => $string_value:literal
            ),*
            $(, | $other:ident)?
            $(,)? // Optional trailing comma
        }
    ) => {
        pastey::paste! {
            #[doc = "Value for the `" $value_name "` css property"]
            #[doc = ""]
            #[doc = "<" $mdn_url ">"]
            $(#[$enum_meta])*
            #[derive(Clone, PartialEq, Eq, Hash, Debug)]
            #[cfg_attr(all(test, not(target_arch="wasm32")), derive(proptest_derive::Arbitrary))]
            pub enum $name {
                $(
                    $(#[$variant_meta])*
                    #[doc = "`" $string_value "`"]
                    $variant,
                )*
                $(
                    #[doc = "Custom value"]
                    $other(String),
                )?
            }

            impl IntoCss for $name {
                #[inline]
                fn into_css(self) -> String {
                    match self {
                        $(
                            Self::$variant => $string_value.into(),
                        )*
                        $(
                            Self::$other(value) => value.into()
                        )?
                    }
                }
            }

            impl CssPropertyValue for $name {
                type Kind = $name;
            }
        }
    }
}

/// Define a zero-sized css value struct
macro_rules! zero_sized_value {
    (struct $name:ident => $value:literal) => {
        pastey::paste! {
            #[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
            #[cfg_attr(
                all(test, not(target_arch = "wasm32")),
                derive(proptest_derive::Arbitrary)
            )]
            #[doc = "`" $value "`"]
            pub struct $name;

            impl IntoCss for $name {
                #[inline]
                fn into_css(self) -> String {
                    $value.into()
                }
            }
            impl CssPropertyValue for $name {
                type Kind = $name;
            }
        }
    };
}

/// A wide keyword is valid in every property.
/// We make it generic to make the trait system work out.
///
/// <https://developer.mozilla.org/docs/Web/CSS/CSS_Values_and_Units/CSS_data_types#css-wide_keywords>
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum WideKeyword<K> {
    /// `initial`
    Initial,
    /// `inherit`
    Inherit,
    /// `revert`
    Revert,
    /// `revert-layer`
    RevertLayer,
    /// `unset`
    Unset,
    /// This is purely here for the phantom data
    Phantom(PhantomData<K>, std::convert::Infallible),
}
impl<K> IntoCss for WideKeyword<K> {
    #[inline]
    fn into_css(self) -> String {
        match self {
            Self::Initial => "initial".into(),
            Self::Inherit => "inherit".into(),
            Self::Revert => "revert".into(),
            Self::RevertLayer => "revert-layer".into(),
            Self::Unset => "unset".into(),
        }
    }
}

impl<K> CssPropertyValue for WideKeyword<K> {
    type Kind = K;
}

define_enum! {
    #[derive(Copy)]
    enum ContentPosition,
    "align-*",
    "https://www.w3.org/TR/css-align-3/#typedef-content-position",
    {
        Center => "center",
        Start => "start",
        End => "end",
        FlexStart => "flex-start",
        FlexEnd => "flex-end",
    }
}

define_enum! {
    #[derive(Copy)]
    enum BaselinePosition,
    "align-*",
    "https://www.w3.org/TR/css-align-3/#typedef-baseline-position",
    {
        Baseline => "baseline",
        First => "first baseline",
        Last => "last baseline",
    }
}

define_enum! {
    #[derive(Copy)]
    enum ContentDistribution,
    "align-*",
    "https://www.w3.org/TR/css-align-3/#typedef-content-distribution",
    {
        SpaceBetween => "space-between",
        SpaceAround => "space-around",
        SpaceEvenly => "space-evenly",
    }
}

define_enum! {
    #[derive(Copy)]
    enum SelfPosition,
    "align-*",
    "https://www.w3.org/TR/css-align-3/#typedef-self-position",
    {
        Center => "center",
        Start => "Start",
        End => "End",
        SelfStart => "self-start",
        SelfEnd => "self-end",
        FlexStart => "flex-start",
        FlexEnd => "flex-end"
    }
}

zero_sized_value!(struct Normal => "normal");
zero_sized_value!(struct Auto => "auto");
zero_sized_value!(struct Stretch => "stretch");

/// <https://developer.mozilla.org/en-US/docs/Web/CSS/align-content#safe>
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(
    all(test, not(target_arch = "wasm32")),
    derive(proptest_derive::Arbitrary)
)]
pub enum OverflowPosition<T> {
    /// `safe ...`
    Safe(T),
    /// `unsafe ...`
    Unsafe(T),
}

impl<T: IntoCss> IntoCss for OverflowPosition<T> {
    fn into_css(self) -> String {
        let (prefix, value) = match self {
            Self::Safe(value) => ("safe", value),
            Self::Unsafe(value) => ("unsafe", value),
        };

        format!("{prefix} {}", value.into_css())
    }
}

impl<T: CssPropertyValue> CssPropertyValue for OverflowPosition<T> {
    type Kind = Self;
}

define_enum!(
    #[derive(Copy)]
    enum StepsJump,
    "steps(...)",
    "https://developer.mozilla.org/en-US/docs/Web/CSS/easing-function/steps#step-position",
    {
        Start => "jump-start",
        End => "jump-end",
        None => "jump-none",
        Both => "jump-both",
    }
);

/// <https://developer.mozilla.org/en-US/docs/Web/CSS/easing-function>
#[derive(Clone, Debug)]
#[cfg_attr(
    all(test, not(target_arch = "wasm32")),
    derive(proptest_derive::Arbitrary)
)]
pub enum EasingFunction {
    /// `linear()` - <https://developer.mozilla.org/en-US/docs/Web/CSS/easing-function/linear>
    #[cfg_attr(all(test, not(target_arch = "wasm32")), proptest(skip))]
    Linear(Vec<(f32, Option<units::Percentage>, Option<units::Percentage>)>),
    /// `cubic-bezier` - <https://developer.mozilla.org/en-US/docs/Web/CSS/easing-function/cubic-bezier>
    CubicBezier {
        /// First control point - (x, y)
        point1: (f32, f32),
        /// Second control point - (x, y)
        point2: (f32, f32),
    },
    /// `steps` - <https://developer.mozilla.org/en-US/docs/Web/CSS/easing-function/steps>
    Steps {
        /// The amount of steps
        steps: u32, // technically this has no upper limit, but lightningcss uses 32-bits (i32)
        /// How to handle the start/end
        jump: StepsJump,
    },
}

impl Default for EasingFunction {
    fn default() -> Self {
        Self::Linear(vec![(0.0, None, None), (1.0, None, None)])
    }
}

impl EasingFunction {
    /// `ease`
    pub const EASE: Self = Self::CubicBezier {
        point1: (0.25, 0.1),
        point2: (0.25, 1.0),
    };
    /// `ease-in`
    pub const EASE_IN: Self = Self::CubicBezier {
        point1: (0.42, 0.0),
        point2: (1.0, 1.0),
    };
    /// `ease-out`
    pub const EASE_OUT: Self = Self::CubicBezier {
        point1: (0.0, 0.0),
        point2: (0.58, 1.0),
    };
    /// `ease-in-out`
    pub const EASE_IN_OUT: Self = Self::CubicBezier {
        point1: (0.42, 0.0),
        point2: (0.58, 1.0),
    };

    /// `step-start`
    pub const STEP_START: Self = Self::Steps {
        steps: 1,
        jump: StepsJump::Start,
    };
    /// `step-end`
    pub const STEP_END: Self = Self::Steps {
        steps: 1,
        jump: StepsJump::End,
    };

    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/easing-function/linear>
    pub fn linear(
        points: impl IntoIterator<Item = (f32, Option<units::Percentage>, Option<units::Percentage>)>,
    ) -> Self {
        Self::Linear(points.into_iter().collect())
    }
}

impl IntoCss for EasingFunction {
    fn into_css(self) -> String {
        match self {
            Self::Linear(points) => {
                let mut result = String::from("linear(");
                for point in points {
                    let res = match point {
                        (point, None, None) => write!(result, "{point}"),
                        (point, Some(start), None) => {
                            write!(result, "{point} {}", start.into_css())
                        }
                        (point, Some(start), Some(end)) => {
                            write!(result, "{point} {} {}", start.into_css(), end.into_css())
                        }
                        (point, None, Some(end)) => {
                            log_or_panic!(
                                "Linear point must specify either first or both percentages, cant specify only end."
                            );
                            write!(result, "{point} {}", end.into_css())
                        }
                    };
                    log_or_panic_result!(res, "Failed to write to string (???).");
                    result.push(',');
                }
                result.push(')');
                result
            }
            Self::CubicBezier { point1, point2 } => {
                format!(
                    "cubic-bezier({}, {}, {}, {})",
                    point1.0, point1.1, point2.0, point2.1
                )
            }
            Self::Steps { steps, jump } => {
                format!("steps({}, {})", steps, jump.into_css())
            }
        }
    }
}

/// The `animation-iteration-count` value.
/// <https://developer.mozilla.org/en-US/docs/Web/CSS/animation-iteration-count>
#[derive(Debug, Clone, Copy)]
#[cfg_attr(
    all(test, not(target_arch = "wasm32")),
    derive(proptest_derive::Arbitrary)
)]
pub enum AnimationIterationCount {
    /// Animation repeats forever
    Infinite,
    /// Loop the animation this amount of times.
    /// This supports partial values such as 0.5.
    Finite(f32),
}

impl Default for AnimationIterationCount {
    fn default() -> Self {
        Self::Finite(1.0)
    }
}

impl IntoCss for AnimationIterationCount {
    fn into_css(self) -> String {
        match self {
            Self::Infinite => String::from("infinite"),
            Self::Finite(value) => value.to_string(),
        }
    }
}

define_enum! {
    #[derive(Copy, Default)]
    enum AnimationDirection,
    "animation-direction",
    "https://developer.mozilla.org/en-US/docs/Web/CSS/animation-direction",
    {
        #[default]
        Normal => "normal",
        Reverse => "reverse",
        Alternate => "alternate",
        AlternateReverse => "alternate-reverse"
    }
}

define_enum! {
    #[derive(Copy, Default)]
    enum AnimationFillMode,
    "animation-fill-mode",
    "https://developer.mozilla.org/en-US/docs/Web/CSS/animation-fill-mode",
    {
        #[default]
        None => "none",
        Forwards => "forwards",
        Backwards => "backwards",
        Both => "both"
    }
}

define_enum! {
    #[derive(Copy, Default)]
    enum AnimationState,
    "animation-play-state",
    "https://developer.mozilla.org/en-US/docs/Web/CSS/animation-play-state",
    {
        #[default]
        Running => "running",
        Paused => "paused"
    }
}

define_enum! {
    #[derive(Copy, Default)]
    enum Appearance,
    "appearance",
    "https://developer.mozilla.org/en-US/docs/Web/CSS/appearance",
    {
        #[default]
        Auto => "auto",
        None => "none"
    }
}

define_enum! {
    #[derive(Copy)]
    enum LengthUnit,
    "*",
    "https://developer.mozilla.org/en-US/docs/Web/CSS/length",
    {
        CapitalHeight => "cap",
        Character => "ch",
        FontSize => "em",
        Xheight => "ex",
        IdealCharacter => "ic",
        Lineheight => "lh",
        RootCapHeight => "rcap",
        RootCharacter => "rch",
        RootFontSize => "rem",
        RootXheight => "rex",
        RootIdealCharacter => "ric",
        RootLineheight => "rlh",
        ContainerQueryWidth => "cqw",
        ContainerQueryHeight => "cqh",
        ContainerQueryInlineSize => "cqi",
        ContainerQueryBlockSize => "cqb",
        ContainerQueryMax => "cqmax",
        ContainerQueryMin => "cqmin",
        Pixel => "px",
        CentiMeter => "cm",
        Millimeter => "mm",
        QuarterMillimeter => "Q",
        Inch => "in",
        Pica => "pc",
        Point => "pt",
        ViewportHeight => "vh",
        ViewportWidth => "vw",
        ViewportMax => "vmax",
        ViewportMin => "vmin",
        ViewportBlockAxis => "vb",
        ViewportInlineAxis => "vi",
        SmallViewportHeight => "svh",
        SmallViewportWidth => "svw",
        SmallViewportMax => "svmax",
        SmallViewportMin => "svmin",
        SmallViewportBlockAxis => "svb",
        SmallViewportInlineAxis => "svi",
        LargeViewportHeight => "lvh",
        LargeViewportWidth => "lvw",
        LargeViewportMax => "lvmax",
        LargeViewportMin => "lvmin",
        LargeViewportBlockAxis => "lvb",
        LargeViewportInlineAxis => "lvi",
        DynamicViewportHeight => "dvh",
        DynamicViewportWidth => "dvw",
        DynamicViewportMax => "dvmax",
        DynamicViewportMin => "dvmin",
        DynamicViewportBlockAxis => "dvb",
        DynamicViewportInlineAxis => "dvi",
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use proptest::proptest;

    use super::*;
    use crate::css::assert_valid_css;

    proptest! {
        #[test]
        fn duration_into_css(duration: Duration) {
            let css = duration.into_css();
            let css = format!("h1 {{ animation-duration: {css}; }}");
            assert_valid_css(&css);
        }
    }
}
