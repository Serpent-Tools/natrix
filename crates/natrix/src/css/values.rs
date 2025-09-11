//! Types for the various css values

mod animations;
mod colors;
pub mod units;

use std::fmt::Write;
use std::marker::PhantomData;
use std::time::Duration;

pub use animations::*;
pub use colors::Color;
pub use units::{Length, Percentage};

pub use super::IntoCss;
use crate::css::values::units::Angle;
use crate::error_handling::{log_or_panic, log_or_panic_result};
use crate::type_macros;

/// Force a unwrap to happen at const time.
/// If the value isnt a valid const expression this wont compile.
///
/// This is to allow you to use the various failable constructors with literal values
/// without hawving to disable any lints you have enabled against unwraps/expects
///
/// ```
/// natrix::const_unwrap!(natrix::css::values::Color::rgba(100, 100, 100, 0.5));
/// ```
/// ```compile_fail
/// natrix::const_unwrap!(natrix::css::values::Color::rgba(100, 100, 100, 120.0));
/// ```
#[macro_export]
macro_rules! const_unwrap {
    ($value:expr) => {
        const {
            match $value {
                Some(value) => value,
                None => panic!("`const_unwrap! on None value"),
            }
        }
    };
}

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

impl CssPropertyValue for Duration {
    type Kind = Duration;
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

/// <https://developer.mozilla.org/en-US/docs/Web/CSS/filter-function>
#[derive(Clone, Debug)]
pub struct Filter(String);

impl Filter {
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/filter-function/blur>
    pub fn blur(value: impl CssPropertyValue<Kind = Length>) -> Self {
        Self(format!("blur({})", value.into_css()))
    }

    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/filter-function/brightness>
    pub fn brightness(value: impl CssPropertyValue<Kind = Percentage>) -> Self {
        Self(format!("brightness({})", value.into_css()))
    }

    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/filter-function/contrast>
    pub fn contrast(value: impl CssPropertyValue<Kind = Percentage>) -> Self {
        Self(format!("contrast({})", value.into_css()))
    }

    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/filter-function/drop-shadow>
    pub fn drop_shadow(
        x_offset: impl CssPropertyValue<Kind = Length>,
        y_offset: impl CssPropertyValue<Kind = Length>,
        blur: impl CssPropertyValue<Kind = Length>,
        color: impl CssPropertyValue<Kind = Color>,
    ) -> Self {
        Self(format!(
            "drop-shadow({} {} {} {})",
            x_offset.into_css(),
            y_offset.into_css(),
            blur.into_css(),
            color.into_css(),
        ))
    }

    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/filter-function/grayscale>
    pub fn grayscale(value: impl CssPropertyValue<Kind = Percentage>) -> Self {
        Self(format!("grayscale({})", value.into_css()))
    }

    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/filter-function/hue-rotate>
    pub fn hue_rotate(value: impl CssPropertyValue<Kind = Angle>) -> Self {
        Self(format!("hue-rotate({})", value.into_css()))
    }

    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/filter-function/invert>
    pub fn invert(value: impl CssPropertyValue<Kind = Percentage>) -> Self {
        Self(format!("invert({})", value.into_css()))
    }

    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/filter-function/opacity>
    pub fn opacity(value: impl CssPropertyValue<Kind = Percentage>) -> Self {
        Self(format!("opacity({})", value.into_css()))
    }

    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/filter-function/saturate>
    pub fn saturate(value: impl CssPropertyValue<Kind = Percentage>) -> Self {
        Self(format!("saturate({})", value.into_css()))
    }

    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/filter-function/sepia>
    pub fn sepia(value: impl CssPropertyValue<Kind = Percentage>) -> Self {
        Self(format!("sepia({})", value.into_css()))
    }
}

impl IntoCss for Filter {
    fn into_css(self) -> String {
        self.0
    }
}

impl CssPropertyValue for Filter {
    type Kind = Filter;
}
impl IntoCss for Vec<Filter> {
    fn into_css(self) -> String {
        self.into_iter()
            .map(|filter| filter.0)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl CssPropertyValue for Vec<Filter> {
    type Kind = Filter;
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::css::assert_valid_css;

    impl Arbitrary for Filter {
        type Parameters = ();
        type Strategy = BoxedStrategy<Self>;

        fn arbitrary_with((): ()) -> Self::Strategy {
            // One strategy per constructor; adapt names if yours differ.
            let blur = any::<Length>().prop_map(Filter::blur);
            let brightness = any::<Percentage>().prop_map(Filter::brightness);
            let contrast = any::<Percentage>().prop_map(Filter::contrast);

            let drop_shadow = (
                any::<Length>(),
                any::<Length>(),
                any::<Length>(),
                any::<Color>(),
            )
                .prop_map(|(x, y, b, c)| Filter::drop_shadow(x, y, b, c));

            let grayscale = any::<Percentage>().prop_map(Filter::grayscale);
            let hue_rotate = any::<Angle>().prop_map(Filter::hue_rotate);
            let invert = any::<Percentage>().prop_map(Filter::invert);
            let opacity = any::<Percentage>().prop_map(Filter::opacity);
            let saturate = any::<Percentage>().prop_map(Filter::saturate);
            let sepia = any::<Percentage>().prop_map(Filter::sepia);

            proptest::prop_oneof![
                blur,
                brightness,
                contrast,
                drop_shadow,
                grayscale,
                hue_rotate,
                invert,
                opacity,
                saturate,
                sepia,
            ]
            .boxed()
        }
    }

    proptest! {
        #[test]
        fn duration_into_css(duration: Duration) {
            let css = duration.into_css();
            let css = format!("h1 {{ animation-duration: {css}; }}");
            assert_valid_css(&css);
        }

        #[test]
        fn filter_into_css(filter: Filter) {
            let css = filter.into_css();
            let css = format!("h1 {{ backdrop-filter: {css}; }}");
            assert_valid_css(&css);
        }
    }
}
