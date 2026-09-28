//! Various traits and functions for writing reusable getter closures.
//! Most abstractions here are built around the `Ref` enum.

use std::ops::{Deref, DerefMut};

/// Either a `&T` or a `&mut T`
/// Use to provide generic getters.
///
/// INVARIANT: All closures dealing with these should preserve the enum variant.
/// Meaning a closure that wants to downgrade a reference needs to just return `&T` instead.
/// Natrix assumes all closure of the form `Fn(Ref<T>) -> Ref<R>` Maintain the variant given.
///
/// INVARIANT: `Read` and `Mut` must only be created in render hooks and event handlers.
/// *not* in async contexts or similar, as certain closures created by the framework assume sync
/// invaraints are upheld.
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Ref<'reference, T: ?Sized> {
    /// a `&T`
    Read(&'reference T),
    /// a `&mut T`
    Mut(&'reference mut T),
    /// a `Option<&mut T>` (used in async)
    FaillableMut(Option<&'reference mut T>),
}

impl<'reference, T: ?Sized> Ref<'reference, T> {
    /// Run a given function depending on whether its a `&` or `&mut`.
    /// These need to return the same type.
    #[inline]
    #[must_use]
    pub fn map<R: ?Sized>(
        self,
        read: impl FnOnce(&'reference T) -> &'reference R,
        write: impl FnOnce(&'reference mut T) -> &'reference mut R,
    ) -> Ref<'reference, R> {
        match self {
            Ref::Read(value) => Ref::Read(read(value)),
            Ref::Mut(value) => Ref::Mut(write(value)),
            Ref::FaillableMut(None) => Ref::FaillableMut(None),
            Ref::FaillableMut(Some(value)) => Ref::FaillableMut(Some(write(value))),
        }
    }

    /// Project a value, meaning transform from `Ref<...>` to `Foo<Ref<...>>`
    /// ```rust
    /// # use natrix::prelude::*;
    /// fn foo(maybe_u8: Ref<Option<u8>>) {
    ///     if let Some(value) = maybe_u8.project() {
    ///         println!("{:?}", value.into_read());
    ///     }
    /// }
    /// ```
    #[inline]
    #[must_use]
    pub fn project(self) -> T::Projected<'reference>
    where
        T: Project,
    {
        T::project(self)
    }

    /// dereference the inner value
    #[inline]
    #[must_use]
    pub fn deref(self) -> Ref<'reference, T::Target>
    where
        T: Deref + DerefMut,
    {
        self.map(|value| &**value, |value| &mut **value)
    }
}

impl<'reference, T: ?Sized> From<&'reference T> for Ref<'reference, T> {
    #[inline]
    fn from(value: &'reference T) -> Self {
        Ref::Read(value)
    }
}
impl<'reference, T: ?Sized> From<&'reference mut T> for Ref<'reference, T> {
    #[inline]
    fn from(value: &'reference mut T) -> Self {
        Ref::Mut(value)
    }
}

/// for example `Ref<Option<T>>` to `Option<Ref<T>>`, basically a abstraction over the various
/// `as_mut`/`as_ref` methods.
pub trait Project: Sized {
    /// The result of the projection, should contain `Ref`s with the `'a` lifetime.
    type Projected<'reference>
    where
        Self: 'reference;

    /// Project a `Ref<Self>` to `Self::Projected`
    fn project(value: Ref<'_, Self>) -> Self::Projected<'_>;
}

impl<T> Project for Option<T> {
    type Projected<'reference>
        = Option<Ref<'reference, T>>
    where
        Self: 'reference;

    fn project(value: Ref<'_, Self>) -> Self::Projected<'_> {
        match value {
            Ref::Read(value) => value.as_ref().map(Into::into),
            Ref::Mut(value) => value.as_mut().map(Into::into),
            Ref::FaillableMut(None) => Some(Ref::FaillableMut(None)),
            Ref::FaillableMut(Some(value)) => {
                value.as_mut().map(|value| Ref::FaillableMut(Some(value)))
            }
        }
    }
}

impl<T, E> Project for Result<T, E> {
    type Projected<'reference>
        = Result<Ref<'reference, T>, Ref<'reference, E>>
    where
        Self: 'reference;

    fn project(value: Ref<'_, Self>) -> Self::Projected<'_> {
        match value {
            Ref::Read(value) => value.as_ref().map(Into::into).map_err(Into::into),
            Ref::Mut(value) => value.as_mut().map(Into::into).map_err(Into::into),
            Ref::FaillableMut(None) => Err(Ref::FaillableMut(None)),
            Ref::FaillableMut(Some(value)) => value
                .as_mut()
                .map(|val| Ref::FaillableMut(Some(val)))
                .map_err(|val| Ref::FaillableMut(Some(val))),
        }
    }
}

/// Trait for items that can be downgraded to references.
/// Specifically this must be implemented for *types* that can represent both mutable and immutable
/// references, i.e ones that build on `Ref`, a implementation with a constant `None` in `as_mut`
/// should be considered broken.
///
/// Note, to avoid unwraps in your code for this you can use `RefClosure` apis instead.
/// Which hides the unwrap behind the assumption the closure is well behaved (maintains variant.)
pub trait Downgrade<'reference> {
    /// The `&` version of this type.
    type ReadOutput;

    /// The `&mut` version of this type.
    type MutOutput;

    /// Convert this to a equivalent type with `&`,
    /// Will downgrade a `&mut` if needed.
    /// Might fail if given `Ref::FaillableMut(None)`
    fn into_read(self) -> Option<Self::ReadOutput>;
    /// Convert this to a equivalent type with `&mut`,
    /// Will return `None` if read variant.
    fn into_mut(self) -> Option<Self::MutOutput>;
}

impl<'reference, T: ?Sized> Downgrade<'reference> for Ref<'reference, T> {
    type ReadOutput = &'reference T;
    type MutOutput = &'reference mut T;

    #[inline]
    fn into_read(self) -> Option<Self::ReadOutput> {
        match self {
            Ref::Read(value) => Some(value),
            Ref::Mut(value) => Some(value),
            Ref::FaillableMut(value) => value.map(|x| &*x),
        }
    }

    #[inline]
    fn into_mut(self) -> Option<Self::MutOutput> {
        match self {
            Ref::Read(_) => None,
            Ref::Mut(value) => Some(value),
            Ref::FaillableMut(value) => value,
        }
    }
}

// NOTE: We do not implement `Downgradable` for `&`
// Because a type that always fails to downgrade into `&mut` is not a valid `Downgradble`
impl<'reference, T: ?Sized> Downgrade<'reference> for &'reference mut T {
    type ReadOutput = &'reference T;
    type MutOutput = &'reference mut T;
    #[inline]
    fn into_read(self) -> Option<Self::ReadOutput> {
        Some(self)
    }
    #[inline]
    fn into_mut(self) -> Option<Self::MutOutput> {
        Some(self)
    }
}
impl<'reference, T> Downgrade<'reference> for Option<T>
where
    T: Downgrade<'reference>,
{
    type ReadOutput = Option<T::ReadOutput>;
    type MutOutput = Option<T::MutOutput>;

    fn into_read(self) -> Option<Self::ReadOutput> {
        let result = match self {
            None => None,
            Some(value) => Some(value.into_read()?),
        };
        Some(result)
    }
    fn into_mut(self) -> Option<Self::MutOutput> {
        match self {
            None => Some(None),
            Some(value) => value.into_mut().map(Some),
        }
    }
}
impl<'reference, T, E> Downgrade<'reference> for Result<T, E>
where
    T: Downgrade<'reference>,
    E: Downgrade<'reference>,
{
    type ReadOutput = Result<T::ReadOutput, E::ReadOutput>;
    type MutOutput = Result<T::MutOutput, E::MutOutput>;

    fn into_read(self) -> Option<Self::ReadOutput> {
        Some(match self {
            Ok(value) => Ok(value.into_read()?),
            Err(value) => Err(value.into_read()?),
        })
    }
    fn into_mut(self) -> Option<Self::MutOutput> {
        Some(match self {
            Ok(value) => Ok(value.into_mut()?),
            Err(value) => Err(value.into_mut()?),
        })
    }
}

/// A Ref closure is a closure that takes a `Ref` and return some downgradable value.
/// And allows calling them with normal references and getting normal references back.
///
/// You should generally not use this bounds, and instead opt for the `impl Fn...` syntax.
pub trait RefClosure<'reference, I: ?Sized, T: Downgrade<'reference>> {
    /// Call the read path of this closure.
    /// This will never fail
    ///
    /// INVARIANT: Must not be called from async, use `call_failable`
    fn call_read(&self, value: &'reference I) -> T::ReadOutput;

    /// Call the mut part of this path.
    /// This will panic if the closure returns `Ref::Read` event if given a `Ref::Mut`
    /// (Which shouldnt happen for any well behaving implementation)
    ///
    /// INVARIANT: Must not be called from async, use `call_failable`
    fn call_mut(&self, value: &'reference mut I) -> T::MutOutput;

    /// Call the mut part of this path, but return `None` if any earlier invariants (like guards),
    /// are no longer valid.
    fn call_failable(&self, value: &'reference mut I) -> Option<T::MutOutput>;
}
impl<'reference, I, T, F> RefClosure<'reference, I, T> for F
where
    F: Fn(Ref<'reference, I>) -> T,
    T: Downgrade<'reference>,
    I: 'reference + ?Sized,
{
    #[expect(clippy::unreachable, reason = "Core invariant.")]
    fn call_read(&self, value: &'reference I) -> T::ReadOutput {
        if let Some(value) = self(Ref::Read(value)).into_read() {
            value
        } else {
            unreachable!("Closure didnt return Read compatible result when given `Ref::Read`");
        }
    }

    #[expect(clippy::unreachable, reason = "Core invariant.")]
    fn call_mut(&self, value: &'reference mut I) -> T::MutOutput {
        if let Some(value) = self(Ref::Mut(value)).into_mut() {
            value
        } else {
            unreachable!("Closure didnt return Mut compatible result when given `Ref::Mut`");
        }
    }

    fn call_failable(&self, value: &'reference mut I) -> Option<T::MutOutput> {
        self(Ref::FaillableMut(Some(value))).into_mut()
    }
}

/// A "alias trait" for `impl Fn(Ref<S>) -> Ref<R> + Clone + 'static`
pub trait Getter<S: ?Sized, R: ?Sized>:
    for<'reference> Fn(Ref<'reference, S>) -> Ref<'reference, R> + Clone + 'static
{
}
impl<S, R, F> Getter<S, R> for F
where
    R: ?Sized,
    S: ?Sized,
    F: Clone + 'static,
    F: for<'reference> Fn(Ref<'reference, S>) -> Ref<'reference, R>,
{
}

/// Access fields on a `Ref<T>`.
/// `field!(foo.bar.abc)` is equivalent to `foo.map(|foo| &foo.bar.abc, |foo| &mut foo.bar.abc)`
///
/// If you wish to use an expression for the target `Ref` use `()`
/// `field!((some_expression()).bar.abc)`
#[macro_export]
macro_rules! field {
    ($name:ident. $($field:ident).+) => {
        $name.map(|$name| &$name.$($field).+, |$name| &mut $name.$($field).+)
    };
    (($value:expr). $($field:ident).+) => {
        ($value).map(|value| &value.$($field).+, |value| &mut value.$($field).+)
    };
}

/// Clone the given values to be captured by the given closure.
///
/// `with!(move foo |...| ...)` is the same as `let foo = foo.clone(); move |...| ...`
/// For multiple captures use `()` like `with!(move (foo, bar) |...| ...)`
#[macro_export]
macro_rules! with {
    (
        move ($($arg:ident),*)
        $($closure:tt)+
    ) =>{{
        $(
            let $arg = $arg.clone();
        )*
        move $($closure)+
    }};
    (
        move $arg:ident
        $($closure:tt)+
    ) =>{{
        $crate::with!(move ($arg) $($closure)*)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Foo {
        value: u8,
    }

    #[test]
    fn map_read_keeps_read() {
        let x = Foo { value: 10 };
        let y = Ref::Read(&x).map(|val| &val.value, |val| &mut val.value);
        assert_eq!(y, Ref::Read(&x.value));
    }

    #[test]
    fn map_mut_keeps_mut() {
        let mut x = Foo { value: 10 };
        let y = Ref::Mut(&mut x).map(|val| &val.value, |val| &mut val.value);
        assert_eq!(y.into_mut(), Some(&mut 10));
    }

    #[test]
    fn field_direct() {
        let x = Foo { value: 10 };
        let borrow = Ref::Read(&x);
        let value = field!(borrow.value);
        assert_eq!(value.into_read(), Some(&10));
    }

    #[test]
    fn field_expr() {
        let x = Foo { value: 10 };
        let value = field!((Ref::Read(&x)).value);
        assert_eq!(value.into_read(), Some(&10));
    }

    fn identify(func: impl Fn(Ref<u8>) -> Ref<u8>) -> impl Fn(Ref<u8>) -> Ref<u8> {
        func
    }

    #[test]
    fn call_read() {
        let getter = identify(|value| value);
        let x = 10;
        assert_eq!(getter.call_read(&x), &10);
    }

    #[test]
    fn call_mut() {
        let getter = identify(|value| value);
        let mut x = 10;
        *getter.call_mut(&mut x) += 10;
        assert_eq!(x, 20);
    }
}
