//! Convert various values to html attributes

use std::borrow::Cow;

use wasm_bindgen::intern;

use super::html_elements::DeferredFunc;
use crate::error_handling::log_or_panic;
use crate::macro_ref::State;
use crate::prelude::Id;
use crate::reactivity::render_callbacks::{
    ReactiveAttribute,
    SimpleReactive,
    SimpleReactiveResult,
};
use crate::reactivity::state::RenderCtx;
use crate::type_macros;

/// The result of apply attribute
pub(crate) enum AttributeResult<C: State> {
    /// The attribute should be set
    SetIt(Option<Cow<'static, str>>),
    /// The attribute requires state
    IsDynamic(DeferredFunc<C>),
}

/// A trait for using a arbitrary type as a attribute value.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid attribute value.",
    note = "Try converting the value to a string"
)]
pub trait ToAttribute<C: State>: 'static {
    /// The kind of attribute output this is
    type AttributeKind;

    /// Return the attribute value, or a deferred function.
    fn calc_attribute(self, name: &'static str, node: &web_sys::Element) -> AttributeResult<C>;
}

/// A attribute that is a integer
pub struct Integer;

/// A attribute that is a float
pub struct Float;

/// generate a `ToAttribute` implementation for a string type
macro_rules! attribute_string {
    ($t:ty, $cow:expr) => {
        impl<C: State> ToAttribute<C> for $t {
            type AttributeKind = String;

            #[inline]
            fn calc_attribute(
                self,
                _name: &'static str,
                _node: &web_sys::Element,
            ) -> AttributeResult<C> {
                AttributeResult::SetIt(Some(($cow)(self)))
            }
        }
    };
}

type_macros::strings!(attribute_string);

impl<C: State> ToAttribute<C> for char {
    type AttributeKind = char;

    #[inline]
    fn calc_attribute(self, _name: &'static str, _node: &web_sys::Element) -> AttributeResult<C> {
        AttributeResult::SetIt(Some(Cow::from(self.to_string())))
    }
}

/// generate `ToAttribute` for a numeric
macro_rules! attribute_numeric {
    ($t:ident, $fmt:ident, $name:ident) => {
        impl<C: State> ToAttribute<C> for $t {
            type AttributeKind = $name;

            #[inline]
            fn calc_attribute(
                self,
                _name: &'static str,
                _node: &web_sys::Element,
            ) -> AttributeResult<C> {
                let mut buffer = $fmt::Buffer::new();
                let result = buffer.format(self);

                AttributeResult::SetIt(Some(Cow::from(result.to_string())))
            }
        }
    };
}

type_macros::numerics!(attribute_numeric);

impl<C: State> ToAttribute<C> for bool {
    type AttributeKind = bool;

    #[inline]
    fn calc_attribute(self, _name: &'static str, _node: &web_sys::Element) -> AttributeResult<C> {
        AttributeResult::SetIt(self.then(|| Cow::from("")))
    }
}

impl<C: State, T: ToAttribute<C>> ToAttribute<C> for Option<T> {
    type AttributeKind = T::AttributeKind;

    #[inline]
    fn calc_attribute(self, name: &'static str, node: &web_sys::Element) -> AttributeResult<C> {
        if let Some(inner) = self {
            inner.calc_attribute(name, node)
        } else {
            AttributeResult::SetIt(None)
        }
    }
}

impl<C: State, T: ToAttribute<C, AttributeKind = K>, E: ToAttribute<C, AttributeKind = K>, K>
    ToAttribute<C> for Result<T, E>
{
    type AttributeKind = K;

    #[inline]
    fn calc_attribute(self, name: &'static str, node: &web_sys::Element) -> AttributeResult<C> {
        match self {
            Ok(inner) => inner.calc_attribute(name, node),
            Err(inner) => inner.calc_attribute(name, node),
        }
    }
}

impl<F, C, R> ToAttribute<C> for F
where
    F: Fn(RenderCtx<C>) -> R + 'static,
    R: ToAttribute<C>,
    C: State,
{
    type AttributeKind = R::AttributeKind;

    #[inline]
    fn calc_attribute(self, name: &'static str, node: &web_sys::Element) -> AttributeResult<C> {
        let node = node.clone();

        AttributeResult::IsDynamic(Box::new(move |ctx, render_state| {
            let hook = SimpleReactive::init_new(
                Box::new(
                    move |ctx, node| match self(ctx).calc_attribute(name, node) {
                        AttributeResult::SetIt(value) => {
                            SimpleReactiveResult::Apply(ReactiveAttribute { name, data: value })
                        }
                        AttributeResult::IsDynamic(inner) => SimpleReactiveResult::Call(inner),
                    },
                ),
                node.clone(),
                ctx,
            );
            render_state.hooks.push(hook);
        }))
    }
}

// TEST: Auto generate tests for validity

/// Defines an enum that represents the value of an enumerated HTML attribute.
macro_rules! define_attribute_enum {
    (
        $(#[$enum_meta:meta])*
        enum $name:ident,
        $attribute_name:literal,
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
            #[doc = "Value for the `" $attribute_name "` attribute"]
            #[doc = ""]
            #[doc = "<" $mdn_url ">"]
            $(#[$enum_meta])*
            #[derive(Clone, PartialEq, Eq, Hash)]
            pub enum $name {
                $(
                    $(#[$variant_meta])*
                    #[doc = "`" $string_value "`"]
                    $variant,
                )*
                $(
                    #[doc = "Custom value"]
                    $other(Cow<'static, str>),
                )?
            }

            impl<C: State> ToAttribute<C> for $name {
                type AttributeKind = $name;

                #[inline]
                fn calc_attribute(self, _name: &'static str, _node: &web_sys::Element) -> AttributeResult<C> {
                    AttributeResult::SetIt(Some(match self {
                        $(
                            Self::$variant => intern($string_value).into(),
                        )*
                        $(
                            Self::$other(value) => value
                        )?
                    }))
                }
            }
        }
    };
}

/// Impl `ToAttribute` for a vec for another attribute using a space separated list
#[macro_export]
macro_rules! impl_to_attribute_for_vec {
    ($T:ty) => {
        impl<C: State> ToAttribute<C> for Vec<$T> {
            type AttributeKind = $T;

            #[inline]
            fn calc_attribute(
                self,
                name: &'static str,
                node: &web_sys::Element,
            ) -> AttributeResult<C> {
                let result = self
                    .into_iter()
                    .map(|item| {
                        if let AttributeResult::SetIt(Some(value)) =
                            ToAttribute::<C>::calc_attribute(item, name, node)
                        {
                            value
                        } else {
                            log_or_panic!(concat!(
                                "`",
                                stringify!($T),
                                "` produced dynamic value or None"
                            ));
                            Cow::from("")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                AttributeResult::SetIt(Some(Cow::from(result)))
            }
        }
    };
}

/// Define a boolean attribute.
/// This is a attribute thats effectively a bool.
/// but the html spec in its infinite wisdom uses a unique set of enumerated values for
macro_rules! define_bool_attribute {
    ($struct_name:ident, $true_str:tt, $false_str:tt) => {
        /// A boolean-like attribute.
        #[derive(Default, Copy, Clone, PartialEq, Eq, Hash)]
        pub struct $struct_name(pub bool);

        impl<C: State> ToAttribute<C> for $struct_name {
            type AttributeKind = $struct_name;

            fn calc_attribute(
                self,
                _name: &'static str,
                _node: &web_sys::Element,
            ) -> AttributeResult<C> {
                AttributeResult::SetIt(Some(Cow::from(if self.0 {
                    intern($true_str)
                } else {
                    intern($false_str)
                })))
            }
        }
    };
}

// has html ever heard of DRY?!
define_bool_attribute!(TrueFalse, "true", "false");
define_bool_attribute!(YesNo, "yes", "no");
define_bool_attribute!(OnOff, "on", "off");

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum ContentEditable,
    "contenteditable",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Global_attributes/contenteditable#value",
    {
        True => "true",
        #[default]
        False => "false",
        PlaintextOnly => "plaintext-only",
    }
}

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum Direction,
    "dir",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Global_attributes/dir",
    {
        LeftToRight => "ltr",
        RightToLeft => "rtl",
        #[default]
        Auto => "auto",
    }
}

define_attribute_enum! {
    #[derive(Copy)]
    enum EnterkeyHint,
    "enterkeyhint",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Global_attributes/enterkeyhint#values",
    {
        Enter => "enter",
        Done => "done",
        Go => "go",
        Next => "next",
        Previous => "previous",
        Search => "search",
        Send => "send",
    }
}

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum InputMode,
    "inputmode",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Global_attributes/inputmode#values",
    {
        None => "none",
        #[default]
        Text => "text",
        Decimal => "decimal",
        Numeric => "numeric",
        Telephone => "tel",
        Search => "search",
        Email => "email",
        Url => "url",
    }
}

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum PopOver,
    "popover",
    "https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Global_attributes/popover#value",
    {
        #[default]
        Auto => "auto",
        Manual => "manual"
    }
}

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum ReferrerPolicy,
    "referrerpolicy",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/a#referrerpolicy",
    {
        NoReferrer => "no-referrer",
        NoReferrerWhenDowngrade => "no-referrer-when-downgrade",
        Origin => "origin",
        OriginWhenCrossOrigin => "origin-when-cross-origin",
        SameOrigin => "same-origin",
        StrictOrigin => "strict-origin",
        #[default]
        StrictOriginWhenCrossOrigin => "strict-origin-when-cross-origin",
        UnsafeUrl => "unsafe-url"
    }
}

define_attribute_enum! {
    enum Rel,
    "rel",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Attributes/rel",
    {
        NoOpener => "noopener",
        NoReferrer => "noreferrer",
        | Other
    }
}

impl_to_attribute_for_vec!(Rel);

define_attribute_enum! {
    #[derive(Default)]
    enum Target,
    "target",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/a#target",
    {
        #[default]
        This => "_self",
        NewTab => "_blank",
        Parent => "_parent",
        Top => "_top",
        UnfencedTop => "_unfencedTop",
        | Other,
    }
}

define_attribute_enum! {
    #[derive(Copy)]
    enum Shape,
    "shape",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/area#shape",
    {
        Rectangle => "rect",
        Circle => "circle",
        Polygon => "poly",
        Remaining => "default"
    }
}

define_attribute_enum! {
    #[derive(Copy)]
    enum ControlsList,
    "controlslist",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/audio#controlslist",
    {
        NoDownload => "nodownload",
        NoFullscreen => "nofullscreen",
        NoRemotePlayback => "noremoteplayback"
    }
}

impl_to_attribute_for_vec!(ControlsList);

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum CrossOrigin,
    "crossorigin",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Attributes/crossorigin",
    {
        #[default]
        Anonymous => "anonymous",
        UseCredentials => "use-credentials"
    }
}

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum ContentPreload,
    "preload",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/audio#preload",
    {
        None => "none",
        #[default]
        Metadata => "metadata",
        Auto => "auto"
    }
}

define_attribute_enum! {
    #[derive(Copy)]
    enum Command,
    "command",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/button#command",
    {
        ShowModal => "show-modal",
        Close => "close",
        RequestClose => "request-close",
        ShowPopover => "show-popover",
        HidePopover => "hide-popover",
        TogglePopover => "toggle-popover",
    }
}

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum EncodingType,
    "enctype",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/form#enctype",
    {
        #[default]
        FormUrlEncoded => "application/x-www-form-urlencoded",
        MultipartFormData => "multipart/form-data",
        PlainText => "text/plain"
    }
}

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum FormMethod,
    "method",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/form#method",
    {
        Post => "post",
        #[default]
        Get => "get",
        Dialog => "dialog",
        Submit => "submit"
    }
}

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum PopoverAction,
    "popovertargetaction",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/button#popovertargetaction",
    {
        Hide => "hide",
        Show => "show",
        #[default]
        Toggle => "toggle"
    }
}

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum ButtonType,
    "type",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/button#type",
    {
        Submit => "submit",
        Reset => "reset",
        #[default]
        Button => "button"
    }
}

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum AutoCapitalize,
    "autocapitalize",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Global_attributes/autocapitalize",
    {
        #[default]
        Off => "off",
        Sentences => "sentences",
        Words => "words",
        Characters => "characters"
    }
}

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum Loading,
    "loading",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/iframe#loading",
    {
        #[default]
        Eager => "eager",
        Lazy => "lazy",
    }
}

define_attribute_enum! {
    #[derive(Copy)]
    enum SandboxAllow,
    "sandbox",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/iframe#sandbox",
    {
        Downloads => "allow-downloads",
        Forms => "allow-forms",
        Modals => "allow-modals",
        OrientationLock => "allow-orientation-lock",
        PointerLock => "allow-pointer-lock",
        Popups => "allow-popups",
        PopupsToEscapeSandbox => "allow-popups-to-escape-sandbox",
        Presentation => "allow-presentation",
        SameOrigin => "allow-same-origin",
        Scripts => "allow-scripts",
        TopNavigation => "allow-top-navigation",
        TopNavigationByUserActivation => "allow-top-navigation-by-user-activation",
        TopNavigationToCustomProtocols => "allow-top-navigation-to-custom-protocols",
    }
}

impl_to_attribute_for_vec!(SandboxAllow);

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum ImageDecoding,
    "decoding",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/img#decoding",
    {
        Sync => "sync",
        Async => "async",
        #[default]
        Auto => "auto"
    }
}

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum FetchPriority,
    "fetchprioority",
    "https://developer.mozilla.org/docs/Web/API/HTMLImageElement/fetchPriority",
    {
        High => "high",
        Low => "low",
        #[default]
        Auto => "auto"
    }
}

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum ListNumberingKind,
    "type",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/ol#type",
    {
        LowercaseLetters => "a",
        UppercaseLetters => "A",
        LowercaseRoman => "i",
        UppercaseRoman => "I",
        #[default]
        Number => "1"
    }
}

impl<C: State> ToAttribute<C> for Vec<Id> {
    type AttributeKind = Vec<Id>;

    fn calc_attribute(self, _name: &'static str, _node: &web_sys::Element) -> AttributeResult<C> {
        let result = self
            .into_iter()
            .map(|id| id.0)
            .collect::<Vec<_>>()
            .join(" ");
        AttributeResult::SetIt(Some(Cow::Owned(result)))
    }
}

/// Define a stringy enum with a `.render` method
macro_rules! define_stringy_enum {
    (
        enum $name:ident,
        $doc_link:literal,
        {
            $(
                $variant:ident => $value:literal
            ),*
        }
    ) =>{
        pastey::paste! {
            #[doc = "<" $doc_link ">"]
            #[derive(Clone, PartialEq, Eq, Hash, Copy)]
            pub enum $name {
                $(
                    #[doc = "`" $value "`"]
                    $variant,
                )*
            }

            impl $name {
                #[inline]
                fn render(self) -> &'static str {
                    match self {
                        $(
                            Self::$variant => $value,
                        )*
                    }
                }
            }
        }
    };
}

define_stringy_enum! {
    enum GroupingIdentifier,
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Attributes/autocomplete#grouping_identifier",
    {
        Shipping => "shipping",
        Billing => "billing"
    }
}

define_stringy_enum! {
    enum RecipientType,
    "https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Attributes/autocomplete#detail_tokens",
    {
        Home => "home",
        Work => "work",
        Mobile => "mobile",
        Fax => "fax",
        Pager => "page"
    }
}

define_stringy_enum! {
    enum ContactKind,
    "https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Attributes/autocomplete#detail_tokens",
    {
        Telephone => "tel",
        TelephoneCountryCode => "tel-country-code",
        TelephoneNation => "tel-nation",
        TelephoneAreaCode => "tel-area-code",
        TelephoneLocal => "tel-local",
        TelephoneExtension => "tel-extension",
        Email => "email",
        InstantMessaging => "impp"
    }
}

define_stringy_enum! {
    enum AutocompleteKind,
    "https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Attributes/autocomplete#detail_tokens",
    {
        Name => "name",
        HonorificPrefix => "honorific-prefix",
        GivenName => "given-name",
        AdditionalName => "additional-name",
        FamilyName => "family-name",
        HonorificSuffix => "honorific-suffix",
        Nickname => "nickname",
        Username => "username",
        NewPassword => "new-password",
        CurrentPassword => "current-password",
        OneTimeCode => "one-time-code",
        OrganizationTitle => "organization-title",
        Organization => "organization",
        StreetAddress => "street-address",
        AddressLine1 => "address-line1",
        AddressLine2 => "address-line2",
        AddressLine3 => "address-line3",
        AddressLevel4 => "address-level4",
        AddressLevel3 => "address-level3",
        AddressLevel2 => "address-level2",
        AddressLevel1 => "address-level1",
        Country => "country",
        CountryName => "country-name",
        PostalCode => "postal-code",
        PaymentName => "cc-name",
        PaymentGivenName => "cc-given-name",
        PaymentAdditionalName => "cc-additional-name",
        PaymentFamilyName => "cc-family-name",
        PaymentNumber => "cc-number",
        PaymentExpiration => "cc-exp",
        PaymentExpirationMonth => "cc-exp-month",
        PaymentExpirationYear => "cc-exp-year",
        PaymentSecurityCode => "cc-csc",
        PaymentKind => "cc-type",
        TransactionCurrency => "transaction-currency",
        TransactionAmount => "transaction-amount",
        Language => "language",
        Birthday => "bday",
        BirtdayDay => "bday-day",
        BirthdayMonth => "bday-month",
        BirthdayYear => "bday-year",
        Gender => "sex",
        Url => "url",
        Photo => "photo"
    }
}

/// <https://developer.mozilla.org/docs/Web/HTML/Reference/Attributes/autocomplete#token_list_tokens>
pub enum DetailTokenPart {
    /// A contact field, like `home email`
    Contact(RecipientType, ContactKind),
    /// a data field, like `new-password` or `name`
    Data(AutocompleteKind),
}

/// <https://developer.mozilla.org/docs/Web/HTML/Reference/Attributes/autocomplete#token_list_tokens>
pub enum AutoComplete {
    /// Enable autocomplete
    On,
    /// Disable autocomplete
    Off,
    /// A specific kind of autocomplete, such as `username`
    SpecificKind {
        /// Name of custom group (`section-...`)
        group_name: Option<Box<str>>,
        /// Optional grouping into billing or shipping
        grouping: Option<GroupingIdentifier>,
        /// Specific data
        detail: Option<DetailTokenPart>,
        /// <https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Attributes/autocomplete#webauthn>
        web_auth: bool,
    },
}

impl<C: State> ToAttribute<C> for AutoComplete {
    type AttributeKind = AutoComplete;

    #[inline]
    fn calc_attribute(self, _name: &'static str, _node: &web_sys::Element) -> AttributeResult<C> {
        let result = match self {
            AutoComplete::On => Cow::Borrowed(intern("on")),
            AutoComplete::Off => Cow::Borrowed(intern("off")),
            AutoComplete::SpecificKind {
                group_name,
                grouping,
                detail,
                web_auth,
            } => {
                let mut result = String::new();
                if let Some(group_name) = group_name {
                    result.push_str("section-");
                    result.push_str(&group_name);
                    result.push(' ');
                }
                if let Some(grouping) = grouping {
                    result.push_str(grouping.render());
                    result.push(' ');
                }
                if let Some(detail) = detail {
                    match detail {
                        DetailTokenPart::Contact(recpient, kind) => {
                            result.push_str(recpient.render());
                            result.push(' ');
                            result.push_str(kind.render());
                        }
                        DetailTokenPart::Data(kind) => {
                            result.push_str(kind.render());
                        }
                    }
                    result.push(' ');
                }

                if web_auth {
                    result.push_str("webauthn");
                }

                Cow::Owned(result)
            }
        };

        AttributeResult::SetIt(Some(result))
    }
}

impl<C: State> ToAttribute<C> for AutocompleteKind {
    type AttributeKind = AutoComplete;

    #[inline]
    fn calc_attribute(self, _name: &'static str, _node: &web_sys::Element) -> AttributeResult<C> {
        AttributeResult::SetIt(Some(Cow::Borrowed(self.render())))
    }
}

define_attribute_enum! {
    #[derive(Default, Copy)]
    enum Wrap,
    "wrap",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/textarea#wrap",
    {
        Hard => "hard",
        #[default]
        Soft => "soft"
    }
}

define_attribute_enum! {
    #[derive(Copy)]
    enum TableHeadingScope,
    "scope",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/th#scope",
    {
        Row => "row",
        Column => "col",
        RowGroup => "rowgroup",
        ColumnGroup => "colgroup"
    }
}

define_attribute_enum! {
    #[derive(Copy)]
    enum TrackKind,
    "kind",
    "https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/track#kind",
    {
        Subtitles => "subtitles",
        Captions => "captions",
        Chapters => "chapters",
        Metadata => "metadata"
    }
}
