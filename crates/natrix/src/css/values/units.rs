//! Implement the various css units
use crate::css::values::{CssPropertyValue, IntoCss};

/// A css percentage.
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
#[cfg_attr(
    all(test, not(target_arch = "wasm32")),
    derive(proptest_derive::Arbitrary)
)]
pub struct Percentage(pub f32);

impl IntoCss for Percentage {
    fn into_css(self) -> String {
        format!("{}%", self.0)
    }
}

impl CssPropertyValue for Percentage {
    type Kind = Percentage;
}

/// <https://developer.mozilla.org/en-US/docs/Web/CSS/angle>
#[derive(Clone, Copy, Debug)]
#[cfg_attr(
    all(test, not(target_arch = "wasm32")),
    derive(proptest_derive::Arbitrary)
)]
pub enum Angle {
    /// 1/360 of a circle
    Degree(f32),
    /// 1/400 of a circle
    Gradian(f32),
    /// 1/2pi of a circle
    Radian(f32),
    /// 1/1 of a circle
    Turn(f32),
}

impl IntoCss for Angle {
    fn into_css(self) -> String {
        match self {
            Self::Degree(degrees) => format!("{degrees}deg"),
            Self::Gradian(gradian) => format!("{gradian}grad"),
            Self::Radian(radian) => format!("{radian}rad"),
            Self::Turn(turn) => format!("{turn}turn"),
        }
    }
}

impl CssPropertyValue for Angle {
    type Kind = Angle;
}

/// Define the `Length` enum
macro_rules! define_length_enum {
    ($($variant:ident => $value:literal),+ $(,)?) => {
        /// A css `<length>` value,
        #[derive(Clone, Copy, PartialEq, Debug)]
        #[cfg_attr(
            all(test, not(target_arch = "wasm32")),
            derive(proptest_derive::Arbitrary)
        )]
        pub enum Length {
            $(
                #[doc = $value]
                #[doc(alias = $value)]
                $variant(f32)
            ),+
        }

        impl IntoCss for Length {
            fn into_css(self) -> String {
                let (value, suffix) = match self {
                    $(Self::$variant(value) => (value, $value)),+
                };
                format!("{value}{suffix}")
            }
        }
    };
}

impl CssPropertyValue for Length {
    type Kind = Length;
}

define_length_enum! {
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
