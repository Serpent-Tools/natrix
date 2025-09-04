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

        // Manual Arbitrary implementation to avoid large nested unions produced
        // by proptest_derive for big enums. We just generate:
        //   - one f32
        //   - one usize in 0..VARIANT_COUNT
        // and map to the corresponding variant.
        #[cfg(all(test, not(target_arch = "wasm32")))]
        #[expect(clippy::arithmetic_side_effects, unused_assignments, reason="Tests")]
        impl proptest::arbitrary::Arbitrary for Length {
            type Parameters = ();
            type Strategy = proptest::strategy::BoxedStrategy<Length>;

            fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
                use proptest::prelude::*;

                // Count variants at compile time.
                const __VARIANT_COUNT: usize = 0 $(+ { let _ = stringify!($variant); 1 })+;

                // Strategy: (f32, index) -> Length
                (
                    any::<f32>(),
                    0usize .. __VARIANT_COUNT
                )
                    .prop_map(|(v, idx)| {
                        // Map idx to variant without building a big match with explicit numeric literals.
                        // O(#variants) but done only at generation time and remains flat.
                        let mut i = idx;
                        $(
                            if i == 0 {
                                return Length::$variant(v);
                            }
                            i -= 1;
                        )+
                        unreachable!("index out of range (variant mapping logic error)");
                    })
                    .boxed()
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
