//! Css properties

use std::marker::PhantomData;

use super::values;
use crate::css::selectors::IntoSelectorList;
use crate::css::values::{CssPropertyValue, IntoCss};

/// A collection of css rules
#[must_use]
pub struct RuleCollection {
    /// Raw sections of css
    pub(crate) sections: Vec<String>,
}

impl Default for RuleCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleCollection {
    /// Create a new stylesheet
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    /// Add a rule to the stylesheet
    pub fn rule(mut self, selector: impl IntoSelectorList, body: RuleBody) -> Self {
        let selector = selector.into_list().into_css();
        let body = body.into_css();

        let section = format!("{selector}{{{body}}}");
        self.sections.push(section);

        self
    }
}

impl IntoCss for RuleCollection {
    fn into_css(self) -> String {
        self.sections.join("")
    }
}

/// A impletor for a property
pub trait Property {
    /// Return the property name
    fn name(self) -> &'static str;
}

/// A marker trait that a given property supports inputs of the given type.
#[diagnostic::on_unimplemented(message = "`{Kind}` is not a valid value for this property")]
pub trait Supports<Kind>: Property {}

/// A css rule body
#[derive(Default, Clone)]
#[must_use]
pub struct RuleBody {
    /// The properties in the rule
    pub properties: Vec<(&'static str, String)>,
}

impl RuleBody {
    /// Create a empty rule body
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add on a property
    ///
    /// All defined properties have helper methods on this struct.
    #[inline]
    pub fn set<V, P>(mut self, property: P, value: V) -> Self
    where
        P: Property,
        V: values::CssPropertyValue,
        P: Supports<V::Kind>,
    {
        self.properties.push((property.name(), value.into_css()));
        self
    }

    /// Add a raw property
    #[inline]
    pub fn raw(mut self, property: &'static str, value: impl Into<String>) -> Self {
        self.properties.push((property, value.into()));
        self
    }
}

impl IntoCss for RuleBody {
    fn into_css(self) -> String {
        let mut result = String::new();
        for (property, value) in self.properties {
            result.push_str(property);
            result.push(':');
            result.push_str(&value);
            result.push(';');
        }

        result
    }
}

/// A css variable lets you both re-use values, and more robustly change css values at runtime that
/// are used across styles.
/// They also allow easier refactor, for example instead of setting a background color directly you
/// can set a css variable, thats used in the background, so if you wanna edit the css to
/// automatically a use a gradient of the background as the border you dont have to update the
/// actual component.
///
/// Css variables should ideally be defined by the `css_var!` macro
#[derive(Clone, Copy)]
pub struct Variable<K> {
    /// The unique name for this css variable.
    name: &'static str,
    /// A phantomdata to hold the type this variable is for
    type_: PhantomData<K>,
}

impl<K> Variable<K> {
    /// Create a new variable with the given name, generally prefer `css_var!` macro over this.
    /// (This is called by the macro.)
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            type_: PhantomData,
        }
    }
}

/// Create a css variable with the given **Kind**
/// Such as `Color`, or `KindNumeric`.
#[macro_export]
macro_rules! css_var {
    () => {
        $crate::css::property::Variable::new($crate::unique_str!())
    };
    ($kind:ty) => {
        $crate::css::property::Variable::<$kind>::new($crate::unique_str!())
    };
}

impl<K> IntoCss for Variable<K> {
    fn into_css(self) -> String {
        format!("var(--{})", super::as_css_identifier(self.name))
    }
}

impl<K> Property for Variable<K> {
    fn name(self) -> &'static str {
        self.name
    }
}

impl<K> CssPropertyValue for Variable<K> {
    type Kind = K;
}
impl<K> Supports<K> for Variable<K> {}

/// Define a property with a specific supported value
macro_rules! property {
    ($name:ident => $target:literal) => {
        pastey::paste! {
            #[doc = "`" $target "` property."]
            #[doc = ""]
            #[doc = "<https://developer.mozilla.org/docs/Web/CSS/" $target ">"]
            #[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
            pub struct $name;

            impl Property for $name {
                #[inline]
                fn name(self) -> &'static str {
                    $target
                }
            }

            impl RuleBody {
                #[doc = "set the `" $target "` property."]
                #[doc = ""]
                #[doc = "<https://developer.mozilla.org/docs/Web/CSS/" $target ">"]
                #[inline]
                pub fn [< $name:snake >]<V>(self, value: V) -> Self
                    where $name: Supports<V::Kind>,
                          V: values::CssPropertyValue,
                {
                    self.set($name, value)
                }
            }
        }
    };
}

/// Generate tests for property support
macro_rules! test_property {
    ($prop:ident, $value:ty, $name:ident) => {
        #[cfg(all(test, not(target_arch = "wasm32")))]
        pastey::paste! {
            proptest::proptest! {
                #[test]
                fn [< test_ $prop:snake _ $name >](value: $value) {
                    let result = RuleCollection::new()
                        .rule(crate::dom::html_elements::TagDiv, RuleBody::new().set($prop, value))
                        .into_css();
                    crate::css::assert_valid_css(&result);
                }
            }
        }
    };
}

/// Define a property support with automatic test generation
macro_rules! support {
    ($prop:ident, $value:ty, $test_name:ident) => {
        impl Supports<$value> for $prop {}
        test_property!($prop, $value, $test_name);
    };
}

/// Generate the support deglation without generating the test
/// Used for types that include generic kinds.
macro_rules! support_no_test {
    ($prop:ident, $value:ty) => {
        impl Supports<$value> for $prop {}
    };
}

property!(AlignContent => "align-content");
support!(AlignContent, values::Normal, normal);
support!(AlignContent, values::Stretch, stretch);
support!(AlignContent, values::ContentPosition, content);
support!(AlignContent, values::BaselinePosition, baseline);
support!(AlignContent, values::ContentDistribution, distribution);
support!(
    AlignContent,
    values::OverflowPosition<values::ContentPosition>,
    overflow
);

property!(AlignSelf => "align-self");
support!(AlignSelf, values::Auto, auto);
support!(AlignSelf, values::Normal, normal);
support!(AlignSelf, values::Stretch, stretch);
support!(AlignSelf, values::BaselinePosition, baseline);
support!(AlignSelf, values::SelfPosition, self);
support!(
    AlignSelf,
    values::OverflowPosition<values::SelfPosition>,
    overflow
);

property!(AlignItems => "align-items");
support!(AlignItems, values::Normal, normal);
support!(AlignItems, values::Stretch, stretch);
support!(AlignItems, values::SelfPosition, self);
support!(AlignItems, values::BaselinePosition, baseline);
support!(
    AlignItems,
    values::OverflowPosition<values::SelfPosition>,
    overflow
);

property!(All => "all");
support_no_test!(All, ());

property!(Animation => "animation");
support!(Animation, values::Animation, single);

property!(Appearance => "appearance");
support!(Appearance, values::Auto, auto);
support!(Appearance, values::Appearance, special);

property!(AspectRatio => "aspect-ratio");
support_no_test!(AspectRatio, values::KindNumeric);
test_property!(AspectRatio, f32, f32);
support!(AspectRatio, values::Auto, auto);
support_no_test!(AspectRatio, (values::Auto, values::KindNumeric));
test_property!(AspectRatio, (values::Auto, f32), auto_f32);
support_no_test!(AspectRatio, (values::KindNumeric, values::Auto));
test_property!(AspectRatio, (f32, values::Auto), f32_auto);
