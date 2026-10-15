//! Lenses/Optics for creating re-usable methods.
//!
//! This module contains the `Optic` type which is used for defining re-usable components.
//! ```rust
//! use natrix::prelude::*;
//!
//! fn increment_button<S: State>(value: impl Lens<&'static S, Target = u32>) -> impl Element<S> {
//!     e::button()
//!         .text(move |mut ctx: RenderCtx<S>| *value.get(&ctx))
//!         .on::<events::Click>(move |mut ctx: EventCtx<S>, _| {
//!             *value.get_mut(&mut ctx) += 1;
//!         })
//! }
//! ```

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

/// A "shape" of value, specifically this is used to describe both read and mutable values,
/// Its core implemenetaiton is for `&T`, which specializes into `&'val T` and `&'val mut T`.
///
/// It is also implemented for types such as `Option`, `Result`, etc similarly:
/// `Option<&T>` -> `Option<&'val T>` and `Option<&'val mut T>`
///
/// (Sadly) This can not be recursive past references, in other words while `Option<Option<&T>>` etc
/// work as expected, `&&T` (or `&Option<&T>`, etc) will only specialize the outer most `&`, i.e
/// `&'val &T` and `&'val mut &T`.
///
/// While there is no compile time enforcement that the `Read` and `Mut` associated types are the
/// intended forms, a lot of the other optic signautres will not be compatiable with once where it
/// isnt.
pub trait Shape {
    /// The "read" variant of this shape specialized by the given lifetime, this should include `&'read ...` references.
    type Read<'read>;
    /// The "mut"/"write" variant of this shape specialized by the given lifetime, this should include `&'read mut ...` references.
    type Mut<'write>;
}

impl<T: ?Sized + 'static> Shape for &'static T {
    type Read<'read> = &'read T;
    type Mut<'write> = &'write mut T;
}

impl<T: Shape> Shape for Option<T> {
    type Read<'read> = Option<T::Read<'read>>;
    type Mut<'write> = Option<T::Mut<'write>>;
}

impl<O: Shape, E: Shape> Shape for Result<O, E> {
    type Read<'read> = Result<O::Read<'read>, E::Read<'read>>;
    type Mut<'write> = Result<O::Mut<'write>, E::Mut<'write>>;
}

/// An optic is a generalization of lenses, prisms, etc.
///
/// It can be tought of as a pair of closures, one for each variant of a `Shape`, While strictly
/// speaking these can be used for many different kinds of transformations the most common and
/// indented usecase is as getters into `State` providing both read and write access *to the same
/// location*.
///
/// While not enforced at compile time, most users, and certain natrix code paths, expect/require
/// that `get` and `get_mut` are "identical" except for `&` vs `&mut`, i.e if one returns `Some` so
/// should the other etc.
/// This invariant is handled for you in most optic methods unless stated otherwise.
#[must_use = "Lenses do nothing if not called"]
pub trait Optic<S: Shape>: Copy + Sized + 'static {
    /// The output of this Optic
    type Output: Shape;

    /// Execute the `Read` path of this optic
    #[must_use]
    fn get<'read>(&self, value: S::Read<'read>) -> <Self::Output as Shape>::Read<'read>;

    /// Execute the `Mut` path of this optic
    #[must_use]
    fn get_mut<'write>(&self, value: S::Mut<'write>) -> <Self::Output as Shape>::Mut<'write>;

    /// Chain this optic into another one.
    ///
    /// In general it is easier to use the various trait methods on `Optic`, `ReferenceOptic`, etc.
    /// But certain optics also require the use of this method, such as `field!`, in addition this
    /// can be used to re-use the same optic across multiple "root" optics.
    ///
    /// ```rust
    /// # use natrix::lens::*;
    /// # use natrix::field;
    /// struct Book {
    ///     name: String,
    /// }
    ///
    /// struct SomeState {
    ///     book1: Book,
    ///     book2: Book,
    /// }
    ///
    /// let book_to_name = root::<&Book>().then(field!(Book, .name)).deref();
    /// let book1_name = root::<&SomeState>().then(field!(SomeState, .book1)).then(book_to_name);
    /// let book2_name = root::<&SomeState>().then(field!(SomeState, .book2)).then(book_to_name);
    /// ```
    #[inline]
    fn then<R>(self, next: R) -> impl Optic<S, Output = R::Output>
    where
        R: Optic<Self::Output>,
    {
        Chain(self, next)
    }
}

/// Internal type used for `.then`, that chains two optics together.
#[must_use = "Lenses do nothing if not called"]
#[derive(Debug, Clone, Copy)]
struct Chain<L, R>(L, R);

impl<S, L, R> Optic<S> for Chain<L, R>
where
    S: Shape,
    L: Optic<S>,
    R: Optic<L::Output>,
{
    type Output = R::Output;

    #[inline]
    fn get<'read>(&self, value: <S as Shape>::Read<'read>) -> <Self::Output as Shape>::Read<'read> {
        self.1.get(self.0.get(value))
    }

    #[inline]
    fn get_mut<'write>(
        &self,
        value: <S as Shape>::Mut<'write>,
    ) -> <Self::Output as Shape>::Mut<'write> {
        self.1.get_mut(self.0.get_mut(value))
    }
}

/// Internal optic type for `root`, that returns the input unchanged, but is a `Optic`.  
#[derive(Debug)]
struct Root<T>(PhantomData<T>);
impl<T> Clone for Root<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Root<T> {}

impl<T: Shape + 'static> Optic<T> for Root<T> {
    type Output = T;

    #[inline]
    fn get<'read>(&self, value: <T as Shape>::Read<'read>) -> <Self::Output as Shape>::Read<'read> {
        value
    }

    #[inline]
    fn get_mut<'write>(
        &self,
        value: <T as Shape>::Mut<'write>,
    ) -> <Self::Output as Shape>::Mut<'write> {
        value
    }
}

/// Return a "noop" optic that just returns its input unchange, specifically the type specificied in
/// this types generic. This is useful for starting the chain of a optic by giving you something to
/// call the chaining trait methods on such as `.deref()`.
#[inline]
pub fn root<T: Shape + 'static>() -> impl Optic<T, Output = T> {
    Root::<T>(PhantomData)
}

/// Internal type for `.deref` that calls `.deref`/`.deref_mut`.
#[derive(Clone, Copy, Debug)]
struct DerefOptic;

impl<T: Deref + DerefMut + 'static> Optic<&'static T> for DerefOptic {
    type Output = &'static T::Target;

    #[inline]
    fn get<'read>(
        &self,
        value: <&'static T as Shape>::Read<'read>,
    ) -> <Self::Output as Shape>::Read<'read> {
        &**value
    }

    #[inline]
    fn get_mut<'write>(
        &self,
        value: <&'static T as Shape>::Mut<'write>,
    ) -> <Self::Output as Shape>::Mut<'write> {
        &mut **value
    }
}

/// Trait methods for optics that return a reference, providing functionality such as
/// dereferencing. This roughly matches the concept of a "lens" from traditional optics.
///
/// This also provides a `ref_map` method (sadly a generic `.map` is not possible on `Optic` due to
/// trait/lifetime limitations.)
pub trait Lens<S: Shape>: Optic<S, Output = &'static Self::Target> {
    /// The target type of the reference, i.e the `T` in `&T`.
    ///
    /// In other words the output of the optic is `&Target`.
    type Target: 'static;

    /// Call `.deref`/`.deref_mut` on the value, for example turning `&String` into `&str`, or
    /// `&Signal<i32>` into `&i32` etc.
    ///
    /// WARN: This assumes that a types `Deref` and `DerefMut` agree with eachother, which for most
    /// sensible types they will.
    #[inline]
    fn deref(self) -> impl Optic<S, Output = &'static <Self::Target as Deref>::Target>
    where
        Self::Target: Deref + DerefMut,
    {
        self.then(DerefOptic)
    }

    /// Create a optic that uses the given closures for its read and write paths.
    /// This is useful for a lot of libraries or types that dont have direct support in natrix, for
    /// example `some_lens.ref_map(|val| val.get_foo(), |val| val.get_foo_mut())`.
    ///
    /// WARN: This requires you to manually ophold the optic "same path" invariant.
    #[inline]
    fn ref_map<R: 'static>(
        self,
        read: impl for<'val> Fn(&'val Self::Target) -> &'val R + Copy + 'static,
        write: impl for<'val> Fn(&'val mut Self::Target) -> &'val mut R + Copy + 'static,
    ) -> impl Optic<S, Output = &'static R> {
        self.then(ReferenceMap::new(read, write))
    }

    /// Project the output value, i.e turning `&Option<T>` into `Option<T>`, i.e a prism (in
    /// addition to also working for result and other enums).
    ///
    /// This is very useful if a component needs a optional value and only plans to modify the inner
    /// value, in other words `impl Optic<&'static S, Output=Option<&'static i32>>`, you can provide
    /// such a optic to the component from a wide array of "source" shpes, if you have a
    /// `Signal<Option<i32>>` then its as simple as `lens_to_field.deref().project()`.
    ///
    /// TODO: Mention also projectable signals.
    #[inline]
    fn project(self) -> impl Optic<S, Output = <Self::Target as Projectable>::Result>
    where
        Self::Target: Projectable,
    {
        self.then(ProjectOptic)
    }
}

impl<S, T, O> Lens<S> for O
where
    S: Shape,
    T: 'static,
    O: Optic<S, Output = &'static T>,
{
    type Target = T;
}

/// The actual `Optic` produced by `ref_map`
///
/// in most cases you dont need to construct this directly.
#[derive(Debug, Clone, Copy)]
#[must_use = "Lenses do nothing if not called"]
pub struct ReferenceMap<FRead, FMut>(FRead, FMut);

impl<FRead, FMut> ReferenceMap<FRead, FMut> {
    /// Construct a new `ReferenceMap`, in general you should prefer `ref_map` over this.
    ///
    /// This method exists as its used by the `field!` macro.
    #[inline]
    pub fn new<Input, Output>(read: FRead, write: FMut) -> Self
    where
        FRead: for<'val> Fn(&'val Input) -> &'val Output,
        FMut: for<'val> Fn(&'val mut Input) -> &'val mut Output,
    {
        Self(read, write)
    }
}

impl<Input: 'static, Output: 'static, FRead, FMut> Optic<&'static Input>
    for ReferenceMap<FRead, FMut>
where
    FRead: for<'val> Fn(&'val Input) -> &'val Output + Copy + 'static,
    FMut: for<'val> Fn(&'val mut Input) -> &'val mut Output + Copy + 'static,
{
    type Output = &'static Output;

    #[inline]
    fn get<'read>(
        &self,
        value: <&'static Input as Shape>::Read<'read>,
    ) -> <Self::Output as Shape>::Read<'read> {
        (self.0)(value)
    }

    #[inline]
    fn get_mut<'write>(
        &self,
        value: <&'static Input as Shape>::Mut<'write>,
    ) -> <Self::Output as Shape>::Mut<'write> {
        (self.1)(value)
    }
}

/// Create a `Optic` that acccess the given field of the given type.
/// This optic can then be passed to `.then`.
///
/// The macro is first passed the type, and then the field path (with a leading `.`).
/// `.then(field!(Book, .title))`, you can also access fields of fields: `.then(field!(Book,
/// .user.name))`
#[macro_export]
macro_rules! field {
    ($type:ty, $(.$field:ident)+) => {
        $crate::lens::ReferenceMap::new::<$type, _>(|value| &value$(.$field)+, |value| &mut value$(.$field)+)
    };
}

/// Trait for "projectable" types, mainly types that can project a reference to themself to version
/// of themself containing reference. In other words the `as_ref` methods on `Option`/`Result`.
///
/// This basically represents a "type-native" `Optic<&Self, Output = ...>`, which can be applied
/// using `Optic::project`.
pub trait Projectable {
    /// The output of the projection, analogus to `Optic::Output`
    type Result: Shape;

    /// The `Read`/`Optic::get` path of this projection.
    fn project(&self) -> <Self::Result as Shape>::Read<'_>;

    /// The `Mut`/`Optic::get_mut` path of this projection.
    fn project_mut(&mut self) -> <Self::Result as Shape>::Mut<'_>;
}

impl<T: 'static> Projectable for Option<T> {
    type Result = Option<&'static T>;

    #[inline]
    fn project(&self) -> <Self::Result as Shape>::Read<'_> {
        self.as_ref()
    }

    #[inline]
    fn project_mut(&mut self) -> <Self::Result as Shape>::Mut<'_> {
        self.as_mut()
    }
}

impl<O: 'static, E: 'static> Projectable for Result<O, E> {
    type Result = Result<&'static O, &'static E>;

    #[inline]
    fn project(&self) -> <Self::Result as Shape>::Read<'_> {
        self.as_ref()
    }

    #[inline]
    fn project_mut(&mut self) -> <Self::Result as Shape>::Mut<'_> {
        self.as_mut()
    }
}

/// Optic for using `Projectable`
#[derive(Debug, Clone, Copy)]
struct ProjectOptic;

impl<T> Optic<&'static T> for ProjectOptic
where
    T: Projectable,
{
    type Output = T::Result;

    fn get<'read>(
        &self,
        value: <&'static T as Shape>::Read<'read>,
    ) -> <Self::Output as Shape>::Read<'read> {
        value.project()
    }

    fn get_mut<'write>(
        &self,
        value: <&'static T as Shape>::Mut<'write>,
    ) -> <Self::Output as Shape>::Mut<'write> {
        value.project_mut()
    }
}

// TODO: Option/Result handling

// TODO: Proejctable signals
// TODO: guards
// TODO: ... async

#[cfg(test)]
mod tests {
    use super::{Lens, Optic};

    fn assert_optic<S: super::Shape, T: super::Shape>(_: &impl Optic<S, Output = T>) {}

    #[test]
    fn root_works() {
        let lens = super::root::<&i32>();
        assert_optic::<&i32, &i32>(&lens);

        {
            let mut value = 0;
            assert_eq!(lens.get(&value), &0);
            *lens.get_mut(&mut value) = 10;
            assert_eq!(lens.get(&value), &10);
        }

        {
            let mut value = 0;
            assert_eq!(lens.get(&value), &0);
            *lens.get_mut(&mut value) = 10;
            assert_eq!(lens.get(&value), &10);
        }
    }

    #[test]
    fn deref_works() {
        let lens = super::root::<&String>().deref();
        assert_optic::<&String, &str>(&lens);

        let mut value = String::from("COOL");
        assert_eq!(lens.get(&value), "COOL");
        lens.get_mut(&mut value).make_ascii_lowercase();
        assert_eq!(lens.get(&value), "cool");
    }

    #[test]
    fn ref_map() {
        struct Book {
            name: String,
        }

        let lens = super::root::<&Book>()
            .ref_map(|book| &book.name, |book| &mut book.name)
            .deref();
        assert_optic::<&Book, &str>(&lens);

        let mut value = Book {
            name: String::from("viv"),
        };
        assert_eq!(lens.get(&value), "viv");
        lens.get_mut(&mut value).make_ascii_uppercase();
        assert_eq!(lens.get(&value), "VIV");
    }

    #[test]
    fn field_works() {
        struct Book {
            name: String,
        }

        let lens = super::root::<&Book>().then(field!(Book, .name)).deref();
        assert_optic::<&Book, &str>(&lens);

        let mut value = Book {
            name: String::from("viv"),
        };
        assert_eq!(lens.get(&value), "viv");
        lens.get_mut(&mut value).make_ascii_uppercase();
        assert_eq!(lens.get(&value), "VIV");
    }
}
