//! Shared infrastructure for the various values you can pass to most natrix dom builders.
//!
//! This module handles the core trait, `WebValue`, and its various implementations on standard
//! library types such as `Option` and `Result`.

use crate::type_macros;

/// A "raw" web value, this does not mean a js value, but rather the natrix type all values should
/// collapse to, for example all elements (`HtmlElement`, `String`, `Option<...>`) etc all target
/// `crate::dom::Node`, which is then internally used to construct dom nodes.
pub trait RawWebValue {
    /// The argument needed for the `WebValue` trait to construct the given value.
    ///
    /// Most implementations wont need all these values, they are mainly used for the closure
    /// implementations.
    type Arguments<'arg>;
}

/// A value convertible to the given `RawWebValue` using the values in `RawWebValue::Arguments`.
///
/// This is the trait backing the usage of `Option`, `Result`, strings and numerics as elements, classes,
/// attributes, etc.
pub trait WebValue<T: RawWebValue>: 'static {
    /// The "kind" of this value, this can be any arbitrary rust type and is used for compile time
    /// "unions" for certain users of this trait.
    ///
    /// This type should be "forwarded" by composition such as `Option` and `Result`.
    /// Multiple "edge" types may target the same `Kind`, for example all strings have kind
    /// `String`, and all integer types have kind `Integer`.
    /// Generally one should prefer to minimize the amount of needed kinds, keeping to the amount
    /// needed to express the webs value semantics.
    /// For example if the only users of kinds `A` and `B` both support `A` and `B` they should most
    /// likely be collapsed into one kind, this should both speed up compile times and simplify
    /// trait bounds.
    ///
    /// For example attributes support one "kind" of attribute value, sometimes `Integer` or `Float`, or
    /// sometimes specific values such as `EnterkeyHint`.
    /// This lets those methods restrict the kinds of attributes they take while still letting
    /// composition (`Result`/`Option`) work as expected transparently
    /// (which a normal `arg: EnterkeyHint` bound would not be able to support)
    ///
    /// See the `SupportedBy` trait for more details and examples of how to write bounds against
    /// this (you should not use direct `Kind = ...` bounds).
    type Kind;

    /// Resolve this value into `T` using the provided arguments.
    fn resolve(self, arguments: T::Arguments<'_>) -> T;
}

/// Marks whether the given kind is supported by the given `T`, this allows bounds to handle
/// composite kinds, for example a method might wanna accept integers or strings, so a
/// `Result<i32, &str>` should be accepted as well, this requires the use of `SupportedBy<T>`;
///
/// This trait is implemented for the following types as follows:
/// * `String`: for `Kind = String`
/// * `Integer`: for `Kind = Integer`
/// * `Float`: for `Kind = Float`, and `Kind = Integer`
/// * `Vec<T>`: for `Kind = Vec<T>`, and `Kind = T`, for the list attribute values in `dom::attributes`
///
/// As well as for `(A, B)` where `A` and `B` implement the trait.
///
/// Specifically one would use it as:
/// ```rust
/// # use natrix::web_value::*;
/// # use std::borrow::Cow;
///
/// #[derive(Default)]
/// struct Target;
///
/// impl RawWebValue for Target {
///     type Arguments<'arg> = ();
/// }
///
/// impl From<Cow<'static, str>> for Target {
///     fn from(_value: Cow<'static, str>) -> Self {
///         Self
///     }
/// }
///
/// fn needs_text(argument: impl WebValue<Target, Kind: SupportedBy<String>>) { /* ... */ }
///
/// needs_text("hello");
/// needs_text(String::from("hello"));
/// needs_text(Some("hello"));
/// needs_text(None::<&'static str>);
/// needs_text(Ok::<&'static str, &'static str>("hello"));
/// needs_text(Err::<&'static str, &'static str>("hello"));
/// ```
///
/// For supporting multiple types you need to define a marker type to hang the implementations on:
/// ```rust
/// # use natrix::web_value::*;
/// # use std::borrow::Cow;
/// #
/// # #[derive(Default)]
/// # struct Target;
/// #
/// # impl RawWebValue for Target {
/// #    type Arguments<'arg> = ();
/// # }
/// #
/// # impl From<Cow<'static, str>> for Target {
/// #    fn from(_value: Cow<'static, str>) -> Self {
/// #       Self
/// #   }
/// # }
/// struct MyMethod;
///
/// impl SupportedBy<MyMethod> for String {};
/// impl SupportedBy<MyMethod> for Integer {};
///
/// fn my_method(argument: impl WebValue<Target, Kind: SupportedBy<MyMethod>>) { /* ... */ }
///
/// my_method("hello");
/// my_method(10_i32);
/// my_method(Ok::<i32, &str>(10));
/// my_method(Err::<i32, &str>("hello"));
/// ```
///
/// And naturally anything of other shapes will be rejected:
/// ```rust,compile_fail
/// # use natrix::web_value::*;
/// # use std::borrow::Cow;
/// #
/// # #[derive(Default)]
/// # struct Target;
/// #
/// # impl RawWebValue for Target {
/// #     type Arguments<'arg> = ();
/// # }
/// #
/// # impl From<Cow<'static, str>> for Target {
/// #     fn from(_value: Cow<'static, str>) -> Self {
/// #         Self
/// #     }
/// # }
/// #
/// # fn needs_text(argument: impl WebValue<Target, Kind: SupportedBy<String>>) { /* ... */ }
/// needs_text(10_i32);
/// ```
/// ```rust,compile_fail
/// # use natrix::web_value::*;
/// # use std::borrow::Cow;
/// #
/// # #[derive(Default)]
/// # struct Target;
/// #
/// # impl RawWebValue for Target {
/// #     type Arguments<'arg> = ();
/// # }
/// #
/// # impl From<Cow<'static, str>> for Target {
/// #     fn from(_value: Cow<'static, str>) -> Self {
/// #         Self
/// #     }
/// # }
/// #
/// # fn needs_text(argument: impl WebValue<Target, Kind: SupportedBy<String>>) { /* ... */ }
/// needs_text(Some(10_i32));
/// ```
/// ```rust,compile_fail
/// # use natrix::web_value::*;
/// # use std::borrow::Cow;
/// #
/// # #[derive(Default)]
/// # struct Target;
/// #
/// # impl RawWebValue for Target {
/// #    type Arguments<'arg> = ();
/// # }
/// #
/// # impl From<Cow<'static, str>> for Target {
/// #    fn from(_value: Cow<'static, str>) -> Self {
/// #       Self
/// #   }
/// # }
/// # struct MyMethod;
/// #
/// # impl SupportedBy<MyMethod> for String {};
/// # impl SupportedBy<MyMethod> for Integer {};
/// #
/// # fn my_method(argument: impl WebValue<Target, Kind: SupportedBy<MyMethod>>) { /* ... */ }
/// my_method(10.0_f32);
/// ```
/// ```rust,compile_fail
/// # use natrix::web_value::*;
/// # use std::borrow::Cow;
/// #
/// # #[derive(Default)]
/// # struct Target;
/// #
/// # impl RawWebValue for Target {
/// #    type Arguments<'arg> = ();
/// # }
/// #
/// # impl From<Cow<'static, str>> for Target {
/// #    fn from(_value: Cow<'static, str>) -> Self {
/// #       Self
/// #   }
/// # }
/// # struct MyMethod;
/// #
/// # impl SupportedBy<MyMethod> for String {};
/// # impl SupportedBy<MyMethod> for Integer {};
/// #
/// # fn my_method(argument: impl WebValue<Target, Kind: SupportedBy<MyMethod>>) { /* ... */ }
/// // Even tho the value we pass would be supported, the result here has a float
/// // (which is not supported) as its error variant
/// my_method(Ok::<&str, f32>("hello"));
/// ```
///
pub trait SupportedBy<T> {}
impl<T, A, B> SupportedBy<T> for (A, B)
where
    A: SupportedBy<T>,
    B: SupportedBy<T>,
{
}

impl SupportedBy<String> for String {}
impl SupportedBy<Integer> for Integer {}
impl SupportedBy<Float> for Float {}
impl SupportedBy<Float> for Integer {}

impl<T, O, E> WebValue<T> for Result<O, E>
where
    T: RawWebValue,
    O: WebValue<T>,
    E: WebValue<T>,
{
    type Kind = (O::Kind, E::Kind);

    fn resolve(self, arguments: T::Arguments<'_>) -> T {
        match self {
            Self::Ok(ok) => ok.resolve(arguments),
            Self::Err(err) => err.resolve(arguments),
        }
    }
}

impl<T, V> WebValue<T> for Option<V>
where
    T: RawWebValue + Default,
    V: WebValue<T>,
{
    type Kind = V::Kind;

    fn resolve(self, arguments: T::Arguments<'_>) -> T {
        match self {
            Some(value) => value.resolve(arguments),
            None => T::default(),
        }
    }
}

impl<T> WebValue<T> for T
where
    T: RawWebValue + 'static,
{
    type Kind = T;

    fn resolve(self, _arguments: T::Arguments<'_>) -> T {
        self
    }
}

/// A web value that is a integer
pub struct Integer;

/// A web value that is a float
pub struct Float;

/// generate a `WebValue` implementation for a string type
macro_rules! webvalue_string {
    ($t:ty, $cow:expr) => {
        impl<T> WebValue<T> for $t
        where
            T: RawWebValue + From<::std::borrow::Cow<'static, str>>,
        {
            type Kind = String;

            #[inline]
            fn resolve(self, _arguments: T::Arguments<'_>) -> T {
                T::from(($cow)(self))
            }
        }
    };
}

type_macros::strings!(webvalue_string);

/// generate a `WebValue` implementation for a numeric type
macro_rules! webvalue_numeric {
    ($t:ident,Float) => {
        impl<T> WebValue<T> for $t
        where
            T: RawWebValue + From<::std::borrow::Cow<'static, str>>,
        {
            type Kind = Float;

            #[inline]
            fn resolve(self, _arguments: T::Arguments<'_>) -> T {
                let mut buffer = ryu::Buffer::new();
                let result = buffer.format(self);

                T::from(::std::borrow::Cow::Owned(result.to_string()))
            }
        }
    };
    ($t:ident,Integer) => {
        impl<T> WebValue<T> for $t
        where
            T: RawWebValue + From<::std::borrow::Cow<'static, str>>,
        {
            type Kind = Integer;

            #[inline]
            fn resolve(self, _arguments: T::Arguments<'_>) -> T {
                let mut buffer = std::fmt::NumBuffer::new();
                let result = self.format_into(&mut buffer);

                T::from(::std::borrow::Cow::Owned(result.to_string()))
            }
        }
    };
}

type_macros::numerics!(webvalue_numeric);
