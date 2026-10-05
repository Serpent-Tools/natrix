//! Css selectors

use std::borrow::Cow;
use std::ops::Deref;

use crate::css::IntoCss;
use crate::dom::html_elements::MaybeDeferred;
use crate::dom::{Attribute, ClassName};
use crate::error_handling::log_or_panic_assert;
use crate::web_value::{SupportedBy, WebValue};

/// A list of selectors (`,`)
#[derive(Debug, Clone)]
pub struct SelectorList(pub Vec<FinalizedSelector>);

/// Create a Selector list of the given elements
///
/// This supports being passed different types of selectors, as it will convert all of them to
/// `FinalizedSelector` automatically
///
/// ```rust
/// # use natrix::prelude::*;
/// # use natrix::selector_list;
/// let _ = selector_list![
///     e::TagDiv, // This is of type `TagDiv`
///     e::TagDiv.child(e::TagH1) // This is of type `CompoundSelector`
/// ];
/// ```
#[macro_export]
macro_rules! selector_list {
    [$($element:expr),*] => {{
        use $crate::css::selectors::IntoFinalizedSelector;
        $crate::css::selectors::SelectorList(vec![
            $(
                $element.into_finalized()
            ),*
        ])
    }};
}

impl IntoCss for SelectorList {
    fn into_css(self) -> String {
        let result: Vec<_> = self
            .0
            .into_iter()
            .map(FinalizedSelector::into_css)
            .collect();

        result.join(",")
    }
}

#[derive(Debug, Clone)]
/// A finalized selector that cant be modified more
pub struct FinalizedSelector {
    /// The selector
    pub head: ComplexSelector,
    /// The pseudo element
    pub element: Option<Box<str>>,
}

impl IntoCss for FinalizedSelector {
    fn into_css(self) -> String {
        let mut result = self.head.into_css();
        if let Some(element) = self.element {
            result.push_str("::");
            result.push_str(&element);
        }
        result
    }
}

/// A combination of various selectors (`h1 h2 > h3`)
#[derive(Debug, Clone)]
pub struct ComplexSelector {
    /// The first selector
    pub first: CompoundSelector,
    /// The rest with their combinators
    pub tail: Vec<(Combinator, CompoundSelector)>,
}

impl IntoCss for ComplexSelector {
    fn into_css(self) -> String {
        let Self { first, tail } = self;

        let mut result = first.into_css();
        for (combinator, selector) in tail {
            result.push_str(&combinator.into_css());
            result.push_str(&selector.into_css());
        }

        result
    }
}

/// A combinator
/// <https://developer.mozilla.org/docs/Web/CSS/CSS_selectors#combinators_and_separators>
#[derive(Debug, Copy, Clone)]
pub enum Combinator {
    /// +
    NextSibling,
    /// >
    DirectChild,
    /// ~
    SubsequentSibling,
    /// ` `
    Descendant,
}

impl IntoCss for Combinator {
    fn into_css(self) -> String {
        match self {
            Self::NextSibling => "+",
            Self::DirectChild => ">",
            Self::SubsequentSibling => "~",
            Self::Descendant => " ",
        }
        .into()
    }
}

/// A compound selector
#[derive(Debug, Clone)]
pub struct CompoundSelector(pub Vec<SimpleSelector>);

impl IntoCss for CompoundSelector {
    fn into_css(self) -> String {
        self.0.into_iter().map(SimpleSelector::into_css).collect()
    }
}

/// A simple selector
#[derive(Debug, Clone)]
pub enum SimpleSelector {
    /// A css tag
    Tag(Box<str>),
    /// Class
    Class(Box<str>),
    /// A id
    Id(Box<str>),
    /// Pseudo Class
    Pseudo(Box<str>),
}

impl IntoCss for SimpleSelector {
    fn into_css(self) -> String {
        match self {
            Self::Tag(value) => value.into(),
            Self::Class(value) => format!(".{}", super::as_css_identifier(&value)),
            Self::Id(value) => format!("#{}", super::as_css_identifier(&value)),
            Self::Pseudo(value) => format!(":{value}"),
        }
    }
}

/// Define `PseudoClass`
macro_rules! define_pseudo_class {
    (
        $($simple:ident),*;
        $($a_rust:ident => $a_name:literal),*;
        $($c_def:ident($($c_arg:ty),*): $c_pat:pat => $c_expr:expr, $c_doc:literal);*;
    ) => {
        pastey::paste! {
            /// Pseudo classes
            pub enum PseudoClass {
                $(
                    #[doc = "<https://developer.mozilla.org/docs/Web/CSS/:" $simple ">"]
                    [< $simple:camel >]
                ),*,
                $(
                    #[doc = "<https://developer.mozilla.org/docs/Web/CSS/:" $a_name ">"]
                    $a_rust
                ),*,
                $(
                    #[doc = "<https://developer.mozilla.org/docs/Web/CSS/:" $c_doc ">"]
                    $c_def($($c_arg),*)
                ),*,
            }

            impl PseudoClass {
                /// Convert to css
                fn into_css(self) -> String {
                    match self {
                        $(
                            Self::[< $simple:camel >] => stringify!($simple).into()
                        ),*,
                        $(
                            Self::$a_rust => $a_name.into()
                        ),*,
                        $(
                            Self::$c_pat => $c_expr
                        ),*,
                    }
                }
            }
        }
    };
}

/// Direction of text
pub enum Direction {
    /// Left to right
    LeftToRight,
    /// Right to left
    RightToLeft,
}

impl Direction {
    /// The short version
    fn short(self) -> &'static str {
        match self {
            Self::LeftToRight => "ltr",
            Self::RightToLeft => "rtl",
        }
    }
}

/// The argument for the `nth-...` pseudo classes
/// <https://developer.mozilla.org/docs/Web/CSS/:nth-child#syntax>
#[must_use]
pub struct NthArgument {
    /// The step
    pub step: i32,
    /// The offset
    pub offset: u32,
    /// The optional `of` selector list
    pub selector: Option<ComplexSelector>,
}

impl NthArgument {
    /// Every event numbered element
    pub const EVEN: Self = NthArgument {
        step: 2,
        offset: 0,
        selector: None,
    };
    /// Every odd numbered element
    pub const ODD: Self = NthArgument {
        step: 2,
        offset: 1,
        selector: None,
    };

    /// Create a new `NthArgument`
    pub fn new(step: i32, offset: u32) -> Self {
        Self {
            step,
            offset,
            selector: None,
        }
    }

    /// Set the selector to use
    pub fn of(mut self, selector: impl IntoComplexSelector) -> Self {
        log_or_panic_assert!(
            self.selector.is_none(),
            "`selector` of NthArgument already set"
        );

        self.selector = Some(selector.into_complex());

        self
    }

    /// Convert to css
    fn into_css(self) -> String {
        let mut result = format!("{}n+{}", self.step, self.offset);
        if let Some(selector) = self.selector {
            result.push_str(" of ");
            result.push_str(&selector.into_css());
        }
        result
    }
}

define_pseudo_class!(
    active, autofill, checked, default, defined, disabled, empty, enabled, first, focus,
    hover, indeterminate, invalid, link, modal, optional, required, root, scope, target,
    valid, visited;

    AnyLink => "any-link",
    FirstChild => "first-child", FirstOfType => "first-of-type",
    FocusVisible => "focus-visible", FocusWithin => "focus-within",
    InRange => "in-range", OutOfRange => "out-of-range",
    LastChild => "last-child", LastOfType => "last-of-type",
    OnlyChild => "only-child", OnlyOfType => "only-of-type",
    PlaceholderShown => "placeholder-shown", PopoverOpen => "popover-open",
    ReadOnly => "read-only", ReadWrite => "read-write",
    UserInvalid => "user-invalid", UserValid => "user-valid";

    Dir(Direction): Dir(dir) => format!("dir({})", dir.short()), "dir";
    Lang(&'static str): Lang(lang) => format!("lang({lang})"), "lang";
    NthChild(NthArgument): NthChild(arg) => format!("nth-child({})", arg.into_css()), "nth-child";
    NthLastChild(NthArgument): NthLastChild(arg) => format!("nth-last-child({})", arg.into_css()), "nth-last-child";
    NthLastOfType(NthArgument): NthLastOfType(arg) => format!("nth-last-of-type({})", arg.into_css()), "nth-last-of-type";
    NthOfType(NthArgument): NthOfType(arg) => format!("nth-of-type({})", arg.into_css()), "nth-of-type";
);

impl IntoSimpleSelector for PseudoClass {
    fn into_simple(self) -> SimpleSelector {
        SimpleSelector::Pseudo(self.into_css().into())
    }
}

/// Define `PseudoClassNested`
macro_rules! define_pseudo_class_nested {
    (
        $($c_def:ident($($c_arg:ty),*): $c_pat:pat => $c_expr:expr, $c_doc:literal);*;
    ) => {
        pastey::paste! {
            /// Pseudo classes
            pub enum PseudoClassNested<S> {
                $(
                    #[doc = "<https://developer.mozilla.org/docs/Web/CSS/:" $c_doc ">"]
                    $c_def($($c_arg),*)
                ),*,
            }

            impl<S: IntoSelectorList> PseudoClassNested<S> {
                /// Convert to css
                fn into_css(self) -> String {
                    match self {
                        $(
                            Self::$c_pat => $c_expr
                        ),*,
                    }
                }
            }
        }
    };
}

define_pseudo_class_nested!(
    Has(S): Has(list) => format!("has({})", list.into_list().into_css()), "has";
    HasWithCombinator(Combinator, S): HasWithCombinator(combinator, list) => format!("has({}{})", combinator.into_css(), list.into_list().into_css()), "has";
    Is(S): Is(list) => format!("is({})", list.into_list().into_css()), "is";
    Not(S): Not(list) => format!("not({})", list.into_list().into_css()), "not";
    Where(S): Where(list) => format!("where({})", list.into_list().into_css()), "where";
);

impl<S: IntoSelectorList> IntoSimpleSelector for PseudoClassNested<S> {
    fn into_simple(self) -> SimpleSelector {
        SimpleSelector::Pseudo(self.into_css().into())
    }
}

/// Convert to a simple selector
#[diagnostic::on_unimplemented(
    message = "{Self} is not a simple selector",
    note = "If you tried to pass a string, use the specific constructors like `class!()`"
)]
pub trait IntoSimpleSelector {
    /// Into a simple selector
    fn into_simple(self) -> SimpleSelector;
}

impl IntoSimpleSelector for SimpleSelector {
    fn into_simple(self) -> SimpleSelector {
        self
    }
}

/// A class generated from the `class` macro
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Class(pub &'static str);

impl<C: crate::reactivity::State> WebValue<ClassName<C>> for Class {
    type Kind = ClassName<C>;

    fn resolve(self, _node: &web_sys::Element) -> ClassName<C> {
        ClassName(MaybeDeferred::Static(Some(self.0.into())))
    }
}

impl IntoSimpleSelector for Class {
    fn into_simple(self) -> SimpleSelector {
        SimpleSelector::Class(self.0.into())
    }
}

impl Deref for Class {
    type Target = str;

    fn deref(&self) -> &'static Self::Target {
        self.0
    }
}

impl From<Class> for &'static str {
    fn from(value: Class) -> Self {
        value.0
    }
}

/// Generate a unique class name
///
/// ```rust
/// # use natrix::prelude::*;
/// const MY_CLASS: Class = natrix::class!();
/// ```
#[macro_export]
macro_rules! class {
    () => {
        $crate::prelude::Class($crate::unique_str!())
    };
}

/// A id generate from the `id` macro
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id(pub &'static str);

impl<C: crate::reactivity::State> WebValue<Attribute<C>> for Id {
    type Kind = Id;

    fn resolve(self, _args: (&'static str, &web_sys::Element)) -> Attribute<C> {
        Attribute::from(Cow::Borrowed(self.0))
    }
}

impl SupportedBy<Id> for Id {}

impl IntoSimpleSelector for Id {
    fn into_simple(self) -> SimpleSelector {
        SimpleSelector::Id(self.0.into())
    }
}

impl Deref for Id {
    type Target = str;

    fn deref(&self) -> &'static Self::Target {
        self.0
    }
}

impl From<Id> for &'static str {
    fn from(value: Id) -> Self {
        value.0
    }
}

/// Generate a unique id name
///
/// ```rust
/// # use natrix::prelude::*;
/// const MY_ID: Id = natrix::id!();
/// ```
#[macro_export]
macro_rules! id {
    () => {
        $crate::prelude::Id($crate::unique_str!())
    };
}

/// For items that can be converted into compound selectors
///
/// This also implements all of the compound selector methods, this lets you do
/// `my_class.and(...).and(...)`
pub trait IntoCompoundSelector: Sized {
    /// Convert into a compound selector
    fn into_compound(self) -> CompoundSelector;

    /// Add another selector to this one *for the same element*
    /// This is equivalent to not having a space in css.
    ///
    /// ```rust
    /// # use natrix::prelude::*;
    /// # use natrix::class;
    /// const BTN: Class = class!();
    /// // div.btn
    /// let _ = e::TagDiv.and(BTN);
    /// ```
    ///
    /// This also enforces the invariant that you tags come first!
    /// ```compile_fail
    /// # use natrix::prelude::*;
    /// # use natrix::class;
    /// const BTN: Class = class!();
    /// // div.btn
    /// let _ = BTN.and(e::TagDiv);
    /// ```
    fn and(self, new: impl IntoSimpleSelector) -> CompoundSelector {
        let mut this = self.into_compound();
        this.0.push(new.into_simple());
        this
    }
}

impl IntoCompoundSelector for CompoundSelector {
    fn into_compound(self) -> CompoundSelector {
        self
    }
}

impl<T: IntoSimpleSelector> IntoCompoundSelector for T {
    fn into_compound(self) -> CompoundSelector {
        CompoundSelector(vec![self.into_simple()])
    }
}

/// Define pseudo element methods
macro_rules! pseudo_elements {
    ($($element:ident),*; $($element_lit:literal => $method:ident),*) => {
        pastey::paste! {
            $(
                #[doc = "<https://developer.mozilla.org/docs/Web/CSS/::" $element ">"]
                fn $element(self) -> FinalizedSelector {
                    FinalizedSelector {
                        head: self.into_complex(),
                        element: Some(stringify!($element).into())
                    }
                }
            )*
            $(
                #[doc = "<https://developer.mozilla.org/docs/Web/CSS/::" $element_lit ">"]
                fn $method(self) -> FinalizedSelector {
                    FinalizedSelector {
                        head: self.into_complex(),
                        element: Some($element_lit.into())
                    }
                }
            )*
        }
    };
}

/// For items that can be converted into complex selectors
///
/// This also implements all of the complex selector methods, this lets you do
/// `my_class.child(...).child(...)`
pub trait IntoComplexSelector: Sized {
    /// Convert into a complex selector
    fn into_complex(self) -> ComplexSelector;

    /// Match a direct child of this selector
    /// i.e css `>`
    /// ```rust
    /// # use natrix::prelude::*;
    /// // p > button
    /// let _ = e::TagP.child(e::TagButton);
    /// ```
    ///
    /// <https://developer.mozilla.org/docs/Web/CSS/Child_combinator>
    fn child(self, child: impl IntoCompoundSelector) -> ComplexSelector {
        let mut this = self.into_complex();
        this.tail
            .push((Combinator::DirectChild, child.into_compound()));
        this
    }

    /// Match any child of this selector.
    /// i.e css space
    /// ```rust
    /// # use natrix::prelude::*;
    /// // p button
    /// let _ = e::TagP.descendant(e::TagButton);
    /// ```
    ///
    /// <https://developer.mozilla.org/docs/Web/CSS/Descendant_combinator>
    fn descendant(self, descendant: impl IntoCompoundSelector) -> ComplexSelector {
        let mut this = self.into_complex();
        this.tail
            .push((Combinator::Descendant, descendant.into_compound()));
        this
    }

    /// Match next sibling of this selector.
    /// i.e css `+`
    /// ```rust
    /// # use natrix::prelude::*;
    /// // p + button
    /// let _ = e::TagP.next_sibling(e::TagButton);
    /// ```
    ///
    /// <https://developer.mozilla.org/docs/Web/CSS/Next-sibling_combinator>
    fn next_sibling(self, sibling: impl IntoCompoundSelector) -> ComplexSelector {
        let mut this = self.into_complex();
        this.tail
            .push((Combinator::NextSibling, sibling.into_compound()));
        this
    }

    /// Match any following sibling of this selector.
    /// i.e css `~`
    /// ```rust
    /// # use natrix::prelude::*;
    /// // p ~ button
    /// let _ = e::TagP.subsequent_sibling(e::TagButton);
    /// ```
    ///
    /// <https://developer.mozilla.org/docs/Web/CSS/Subsequent-sibling_combinator>
    fn subsequent_sibling(self, sibling: impl IntoCompoundSelector) -> ComplexSelector {
        let mut this = self.into_complex();
        this.tail
            .push((Combinator::SubsequentSibling, sibling.into_compound()));
        this
    }

    pseudo_elements!(
        after, before, backdrop, placeholder, cue;
        "file-selector-button" => file_selector_button,
        "target-text" => target_text,
        "first-line" => first_line,
        "first-letter" => first_letter
    );
}

impl IntoComplexSelector for ComplexSelector {
    fn into_complex(self) -> ComplexSelector {
        self
    }
}

impl<T: IntoCompoundSelector> IntoComplexSelector for T {
    fn into_complex(self) -> ComplexSelector {
        ComplexSelector {
            first: self.into_compound(),
            tail: Vec::new(),
        }
    }
}

/// Convert a selector into a finalized one
pub trait IntoFinalizedSelector {
    /// Convert into a finalized one
    fn into_finalized(self) -> FinalizedSelector;
}

impl IntoFinalizedSelector for FinalizedSelector {
    fn into_finalized(self) -> FinalizedSelector {
        self
    }
}

impl<T: IntoComplexSelector> IntoFinalizedSelector for T {
    fn into_finalized(self) -> FinalizedSelector {
        FinalizedSelector {
            head: self.into_complex(),
            element: None,
        }
    }
}

/// Convert a selector into a selector list
pub trait IntoSelectorList {
    /// Convert into a list
    fn into_list(self) -> SelectorList;
}

impl IntoSelectorList for SelectorList {
    fn into_list(self) -> SelectorList {
        self
    }
}

impl<T: IntoFinalizedSelector> IntoSelectorList for T {
    fn into_list(self) -> SelectorList {
        SelectorList(vec![self.into_finalized()])
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use insta::assert_snapshot;
    use static_assertions::{assert_impl_all, assert_not_impl_any};

    use super::*;
    use crate::css::assert_valid_css;
    use crate::dom::html_elements::TagDiv;

    assert_impl_all!(TagDiv: IntoCompoundSelector, IntoComplexSelector, IntoFinalizedSelector, IntoSelectorList);
    assert_impl_all!(Class: IntoSimpleSelector, IntoCompoundSelector, IntoComplexSelector, IntoFinalizedSelector, IntoSelectorList);
    assert_impl_all!(PseudoClass: IntoSimpleSelector, IntoCompoundSelector, IntoComplexSelector, IntoFinalizedSelector, IntoSelectorList);
    assert_impl_all!(PseudoClassNested<TagDiv>: IntoSimpleSelector, IntoCompoundSelector, IntoComplexSelector, IntoFinalizedSelector, IntoSelectorList);
    assert_impl_all!(SimpleSelector: IntoSimpleSelector, IntoCompoundSelector, IntoComplexSelector, IntoFinalizedSelector, IntoSelectorList);
    assert_impl_all!(CompoundSelector: IntoCompoundSelector, IntoComplexSelector, IntoFinalizedSelector, IntoSelectorList);
    assert_impl_all!(ComplexSelector: IntoComplexSelector, IntoFinalizedSelector, IntoSelectorList);
    assert_impl_all!(FinalizedSelector: IntoFinalizedSelector, IntoSelectorList);
    assert_impl_all!(SelectorList: IntoSelectorList);

    // Tags can only be the start of a compound selector, so should not be valid to be inserted in
    // the middle of one.
    assert_not_impl_any!(TagDiv: IntoSimpleSelector);
    assert_not_impl_any!(CompoundSelector: IntoSimpleSelector);
    assert_not_impl_any!(ComplexSelector: IntoSimpleSelector, IntoCompoundSelector);
    assert_not_impl_any!(FinalizedSelector: IntoSimpleSelector, IntoCompoundSelector, IntoComplexSelector);
    assert_not_impl_any!(SelectorList: IntoSimpleSelector, IntoCompoundSelector, IntoComplexSelector, IntoFinalizedSelector);

    macro_rules! assert_valid_and_snapsot {
        ($expr:expr) => {
            let selector = $expr.into_finalized(); // Ensure its correct from the top
            let result = selector.into_css();

            let wrapped = format!("{result} {{}}");
            assert_valid_css(&wrapped);

            assert_snapshot!(stringify!($expr), result, stringify!($expr));
        };
    }

    // We do not want this to actually use `unique_str` in a testing situation.
    const BTN: Class = Class("btn");
    const PROFILE: Id = Id("profile");

    #[test]
    fn cases() {
        assert_valid_and_snapsot!(TagDiv);
        assert_valid_and_snapsot!(BTN);
        assert_valid_and_snapsot!(PROFILE);
        assert_valid_and_snapsot!(TagDiv.and(BTN));
        assert_valid_and_snapsot!(TagDiv.and(PROFILE));
        assert_valid_and_snapsot!(TagDiv.child(BTN));
        assert_valid_and_snapsot!(TagDiv.descendant(BTN));
        assert_valid_and_snapsot!(TagDiv.descendant(BTN).descendant(PROFILE));
        assert_valid_and_snapsot!(TagDiv.next_sibling(BTN));
        assert_valid_and_snapsot!(TagDiv.subsequent_sibling(BTN));
        assert_valid_and_snapsot!(TagDiv.and(BTN).descendant(BTN));
        assert_valid_and_snapsot!(BTN.descendant(TagDiv.and(BTN)));
        assert_valid_and_snapsot!(TagDiv.descendant(TagDiv).descendant(TagDiv));
        assert_valid_and_snapsot!(TagDiv.after());
        assert_valid_and_snapsot!(PROFILE.after());
        assert_valid_and_snapsot!(TagDiv.descendant(BTN).before());
        assert_valid_and_snapsot!(PseudoClass::Hover);
        assert_valid_and_snapsot!(BTN.and(PseudoClass::Hover));
        assert_valid_and_snapsot!(TagDiv.descendant(PseudoClass::Hover));
        assert_valid_and_snapsot!(TagDiv.and(PseudoClassNested::Has(BTN)));
        assert_valid_and_snapsot!(TagDiv.and(PseudoClassNested::HasWithCombinator(
            Combinator::DirectChild,
            BTN
        )));
        assert_valid_and_snapsot!(TagDiv.and(PseudoClassNested::Has(selector_list![BTN, TagDiv])));
        assert_valid_and_snapsot!(TagDiv.and(PseudoClass::NthChild(NthArgument::new(2, 3))));
        assert_valid_and_snapsot!(
            TagDiv.and(PseudoClass::NthChild(NthArgument::new(2, 3).of(BTN)))
        );
    }
}
