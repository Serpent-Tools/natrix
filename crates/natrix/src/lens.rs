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
/// intended forms, a lot of the other optic signatures will not be compatible with once where it
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

/// A `Shape` that resolves to `T` for both read and mut, needed for expressing owned data in a
/// `Optic` (because otherwise the implementation would collidie with every other one).
///
/// NOTE: this is intended for use in `Result` (and similar) such as
/// `Optic<..., Output = Result<&'static str, String>>`,
/// if you only require owned data use a normal closure instead.
pub struct Owned<T>(T);

impl<T> Shape for Owned<T> {
    type Read<'read> = T;
    type Mut<'write> = T;
}

/// An optic is a generalization of lenses, prisms, etc.
///
/// It can be thought of as a pair of closures, one for each variant of a `Shape`, While strictly
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
    ///
    /// # Panics
    /// if the optic path contains guards or similar, and they are moved outside their intended
    /// scope. See `try_get` for alternative.
    #[must_use]
    fn get<'read>(&self, value: S::Read<'read>) -> <Self::Output as Shape>::Read<'read>;

    /// Execute the `Mut` path of this optic
    ///
    /// # Panics
    /// if the optic path contains guards or similar, and they are moved outside their intended
    /// scope. See `try_get` for alternative.
    #[must_use]
    fn get_mut<'write>(&self, value: S::Mut<'write>) -> <Self::Output as Shape>::Mut<'write>;

    /// Execute the `Mut` path of this optic, but return `None` if any part of the chain, mainly
    /// guards, fail. This should be used for async or any other situation where a optic is sorted
    /// outside the normal reactivity tree.
    #[must_use]
    #[inline]
    fn try_get<'write>(
        &self,
        value: S::Mut<'write>,
    ) -> Option<<Self::Output as Shape>::Mut<'write>> {
        Some(self.get_mut(value))
    }

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
    fn then<R>(self, next: R) -> Chain<Self, R>
    where
        R: Optic<Self::Output>,
    {
        Chain(self, next)
    }

    /// Chain a operation into this optic, for example `.perform(lens::ResultOk)`.
    ///
    /// Most operations will have helper methods on various traits such as [`OpticResultExt::ok`]
    /// for [`ResultOk`].
    #[inline]
    fn perform<Op>(self, operation: Op) -> Chain<Self, OperationOptic<Op>>
    where
        Op: Operation<Self::Output>,
        for<'read> <Self::Output as Shape>::Read<'read>:
            OperationTarget<Op, <Op::Output as Shape>::Read<'read>>,
        for<'write> <Self::Output as Shape>::Mut<'write>:
            OperationTarget<Op, <Op::Output as Shape>::Mut<'write>>,
    {
        self.then(OperationOptic(operation))
    }

    /// Call `Into::into` on the value, such as converting from `&T` to `Option<&T>` for functions
    /// that require it.
    ///
    /// ```rust
    /// # use natrix::prelude::*;
    /// # use natrix::lens;
    /// let lens = lens::root::<&'static i32>().then_into::<Option<&'static i32>>();
    ///
    /// let mut value = 10;
    /// if let Some(value) = lens.get_mut(&mut value) {
    ///     *value = 10;
    /// }
    /// assert_eq!(value, 10);
    /// ```
    ///
    /// Similarly to `deref` this assumes the read and write `Into::into` paths are equivalanet,
    /// which they will be for most sensible types.
    // NOTE: called `then_into` to not conflict with `Into::into`
    fn then_into<T: Shape + 'static>(self) -> Chain<Self, OperationOptic<Convert<T>>>
    where
        for<'read> T::Read<'read>: From<<Self::Output as Shape>::Read<'read>>,
        for<'write> T::Mut<'write>: From<<Self::Output as Shape>::Mut<'write>>,
    {
        self.perform(Convert(PhantomData))
    }
}

/// Internal type used for `.then`, that chains two optics together.
#[must_use = "Lenses do nothing if not called"]
#[derive(Debug, Clone, Copy)]
pub struct Chain<L, R>(pub L, pub R);

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

    #[inline]
    fn try_get<'write>(
        &self,
        value: <S as Shape>::Mut<'write>,
    ) -> Option<<Self::Output as Shape>::Mut<'write>> {
        self.1.try_get(self.0.try_get(value)?)
    }
}

/// Internal optic type for `root`, that returns the input unchanged, but is a `Optic`.  
#[must_use = "Lenses do nothing if not called"]
#[derive(Debug)]
pub struct Root<T>(PhantomData<T>);
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

/// Return a "noop" optic that just returns its input unchange, specifically the type specified in
/// this types generic. This is useful for starting the chain of a optic by giving you something to
/// call the chaining trait methods on such as `.deref()`.
#[inline]
pub fn root<T: Shape + 'static>() -> Root<T> {
    Root::<T>(PhantomData)
}

/// Internal operation for `.deref` that calls `.deref`/`.deref_mut`.
#[derive(Clone, Copy, Debug)]
pub struct Dereference;

impl<T: ?Sized + Deref + 'static> Operation<&'static T> for Dereference {
    type Output = &'static T::Target;
}

impl<'read, T: ?Sized + Deref> OperationTarget<Dereference, &'read T::Target> for &'read T {
    #[inline]
    fn execute(self, _op: &Dereference) -> &'read T::Target {
        self
    }
}

impl<'write, T: ?Sized + DerefMut> OperationTarget<Dereference, &'write mut T::Target>
    for &'write mut T
{
    #[inline]
    fn execute(self, _op: &Dereference) -> &'write mut T::Target {
        self
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
    type Target: ?Sized + 'static;

    /// Call `.deref`/`.deref_mut` on the value, for example turning `&String` into `&str`, or
    /// `&Signal<i32>` into `&i32` etc.
    ///
    /// WARN: This assumes that a types `Deref` and `DerefMut` agree with eachother, which for most
    /// sensible types they will.
    #[inline]
    fn deref(self) -> Chain<Self, OperationOptic<Dereference>>
    where
        Self::Target: Deref + DerefMut,
    {
        self.perform(Dereference)
    }

    /// Create a optic that uses the given closures for its read and write paths.
    /// This is useful for a lot of libraries or types that dont have direct support in natrix, for
    /// example `some_lens.ref_map(|val| val.get_foo(), |val| val.get_foo_mut())`.
    ///
    /// WARN: This requires you to manually ophold the optic "same path" invariant.
    #[inline]
    fn ref_map<R, Fread, Fmut>(
        self,
        read: Fread,
        write: Fmut,
    ) -> Chain<Self, OperationOptic<ReferenceMap<Fread, Fmut>>>
    where
        R: ?Sized + 'static,
        Fread: for<'val> Fn(&'val Self::Target) -> &'val R + Copy + 'static,
        Fmut: for<'val> Fn(&'val mut Self::Target) -> &'val mut R + Copy + 'static,
    {
        self.perform(ReferenceMap::new(read, write))
    }

    /// Project the output value, i.e turning `&Option<T>` into `Option<T>`, i.e a prism (in
    /// addition to also working for result and other enums).
    ///
    /// This is very useful if a component needs a optional value and only plans to modify the inner
    /// value, in other words `impl Optic<&'static S, Output=Option<&'static i32>>`, you can provide
    /// such a optic to the component from a wide array of "source" shapes, if you have a
    /// `Signal<Option<i32>>` then its as simple as `lens_to_field.deref().project()`.
    ///
    /// When the signal holds a `State` type, such as `Signal<Option<Book>>`, the signal itself can
    /// be projected with `lens_to_field.project()`. Writes through the resulting optic never mark
    /// the outer signal as changed, only the inner `Book` signals that are actually modified
    /// trigger re-renders. See [`Signal::project_mut`](crate::reactivity::signal::Signal) for
    /// details.
    #[inline]
    #[doc(alias = "as_ref")]
    fn project(self) -> Chain<Self, OperationOptic<Project>>
    where
        Self::Target: Projectable,
    {
        self.perform(Project)
    }
}

impl<S, T, O> Lens<S> for O
where
    S: Shape,
    T: ?Sized + 'static,
    O: Optic<S, Output = &'static T>,
{
    type Target = T;
}

/// The operation performed by `ref_map`.
///
/// In most cases you dont need to construct this directly.
#[derive(Debug, Clone, Copy)]
pub struct ReferenceMap<FRead, FMut>(FRead, FMut);

impl<FRead, FMut> ReferenceMap<FRead, FMut> {
    /// Construct a new `ReferenceMap`, in general you should prefer `ref_map` over this.
    ///
    /// This method exists as its used by the `field!` macro.
    #[inline]
    pub fn new<Input: ?Sized, Output: ?Sized>(read: FRead, write: FMut) -> Self
    where
        FRead: for<'val> Fn(&'val Input) -> &'val Output,
        FMut: for<'val> Fn(&'val mut Input) -> &'val mut Output,
    {
        Self(read, write)
    }
}

impl<Input: ?Sized + 'static, Output: ?Sized + 'static, FRead, FMut> Operation<&'static Input>
    for ReferenceMap<FRead, FMut>
where
    FRead: for<'val> Fn(&'val Input) -> &'val Output + Copy + 'static,
    FMut: for<'val> Fn(&'val mut Input) -> &'val mut Output + Copy + 'static,
{
    type Output = &'static Output;
}

impl<'read, Input: ?Sized, Output: ?Sized, FRead, FMut>
    OperationTarget<ReferenceMap<FRead, FMut>, &'read Output> for &'read Input
where
    FRead: for<'val> Fn(&'val Input) -> &'val Output,
{
    #[inline]
    fn execute(self, op: &ReferenceMap<FRead, FMut>) -> &'read Output {
        (op.0)(self)
    }
}

impl<'write, Input: ?Sized, Output: ?Sized, FRead, FMut>
    OperationTarget<ReferenceMap<FRead, FMut>, &'write mut Output> for &'write mut Input
where
    FMut: for<'val> Fn(&'val mut Input) -> &'val mut Output,
{
    #[inline]
    fn execute(self, op: &ReferenceMap<FRead, FMut>) -> &'write mut Output {
        (op.1)(self)
    }
}

/// Create a `Optic` that access the given field of the given type.
/// This optic can then be passed to `.then`.
///
/// The macro is first passed the type, and then the field path (with a leading `.`).
/// `.then(field!(Book, .title))`, you can also access fields of fields: `.then(field!(Book,
/// .user.name))`
#[macro_export]
macro_rules! field {
    ($type:ty, $(.$field:ident)+) => {
        $crate::lens::OperationOptic(
            $crate::lens::ReferenceMap::new::<$type, _>(|value| &value$(.$field)+, |value| &mut value$(.$field)+)
        )
    };
}

/// Trait for "projectable" types, mainly types that can project a reference to themself to version
/// of themself containing reference. In other words the `as_ref` methods on `Option`/`Result`.
///
/// This basically represents a "type-native" `Optic<&Self, Output = ...>`, which can be applied
/// using `Lens::project`.
pub trait Projectable {
    /// The output of the projection, analogous to `Optic::Output`
    type Result: Shape;

    /// The `Read`/`Optic::get` path of this projection.
    #[doc(alias = "as_ref")]
    fn project(&self) -> <Self::Result as Shape>::Read<'_>;

    /// The `Mut`/`Optic::get_mut` path of this projection.
    #[doc(alias = "as_mut")]
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

/// Operation for using `Projectable`
#[derive(Debug, Clone, Copy)]
pub struct Project;

impl<T: ?Sized + Projectable> Operation<&'static T> for Project {
    type Output = T::Result;
}

impl<'read, T: ?Sized + Projectable> OperationTarget<Project, <T::Result as Shape>::Read<'read>>
    for &'read T
{
    #[inline]
    fn execute(self, _op: &Project) -> <T::Result as Shape>::Read<'read> {
        self.project()
    }
}

impl<'write, T: ?Sized + Projectable> OperationTarget<Project, <T::Result as Shape>::Mut<'write>>
    for &'write mut T
{
    #[inline]
    fn execute(self, _op: &Project) -> <T::Result as Shape>::Mut<'write> {
        self.project_mut()
    }
}

/// A single step of an optic, such as [`Dereference`] or [`ResultOk`], added to an optic with
/// [`Optic::perform`].
///
/// This trait gives the output shape for an input shape, and [`OperationTarget`] implements the
/// step. If the step is the same for both paths a single `OperationTarget` impl covers them, as
/// for [`ResultOk`]. Otherwise implement it for both the `Read` and `Mut` forms of the shape, i.e
/// `&'read T` and `&'write mut T`, as for [`Dereference`].
///
/// Type errors between the two traits only show up once the operation is performed.
///
/// ```rust
/// # use natrix::prelude::*;
/// # use natrix::lens;
/// #[derive(Clone, Copy)]
/// struct AddError(&'static str);
///
/// impl<T: lens::Shape> lens::Operation<Option<T>> for AddError {
///     type Output = Result<T, lens::Owned<&'static str>>;
/// }
///
/// impl<T> lens::OperationTarget<AddError, Result<T, &'static str>> for Option<T> {
///     fn execute(self, op: &AddError) -> Result<T, &'static str> {
///         self.ok_or(op.0)
///     }
/// }
///
/// let my_optic = lens::root::<Option<&'static i32>>().perform(AddError("oh no!"));
/// assert_eq!(my_optic.get(Some(&10)), Ok(&10));
/// assert_eq!(my_optic.get(None), Err("oh no!"));
/// ```
pub trait Operation<Input: Shape>: Copy + 'static {
    /// The output of this operation given the given input shape.
    type Output: Shape;
}

/// Implementation of a [`Operation`], this trait has a slightly unusual generic form in order to
/// make rust be able to reason about bounds using it.
///
/// The first generic is the operation, the output generic is the output of the operation for the
/// given `Self` type. `Self` should be some generic version of `Input` in `Operation`, and `Output`
/// a generic version of `Operation::Output`.
pub trait OperationTarget<Operation, Output> {
    /// Perform the operation on this element, in general the `op` argument will be a ignored ZST,
    /// but it is also possible for a operation to contain some state.
    fn execute(self, op: &Operation) -> Output;
}

/// The `Optic` that performs a single `Operation`, usually constructed using
/// [`Optic::perform`].
#[derive(Debug, Clone, Copy)]
pub struct OperationOptic<Op>(pub Op);

impl<Input, Op> Optic<Input> for OperationOptic<Op>
where
    Input: Shape,
    Op: Operation<Input>,
    for<'read> Input::Read<'read>:
        OperationTarget<Op, <<Op as Operation<Input>>::Output as Shape>::Read<'read>>,
    for<'write> Input::Mut<'write>:
        OperationTarget<Op, <<Op as Operation<Input>>::Output as Shape>::Mut<'write>>,
{
    type Output = <Op as Operation<Input>>::Output;

    #[inline]
    fn get<'read>(
        &self,
        value: <Input as Shape>::Read<'read>,
    ) -> <Self::Output as Shape>::Read<'read> {
        value.execute(&self.0)
    }

    #[inline]
    fn get_mut<'write>(
        &self,
        value: <Input as Shape>::Mut<'write>,
    ) -> <Self::Output as Shape>::Mut<'write> {
        value.execute(&self.0)
    }
}

/// Internal operation for `.then_into` that calls `Into::into`, converting to the shape `T`.
#[derive(Debug)]
pub struct Convert<T>(PhantomData<T>);
impl<T> Copy for Convert<T> {}
impl<T> Clone for Convert<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: Shape, T: Shape + 'static> Operation<S> for Convert<T> {
    type Output = T;
}

impl<S, T, TShape> OperationTarget<Convert<TShape>, T> for S
where
    T: From<S>,
{
    #[inline]
    fn execute(self, _op: &Convert<TShape>) -> T {
        self.into()
    }
}

/// Trait methods for optics returning a (owned) `Result`, for stuff like [`Result::ok`], [`Result::err`], etc.
pub trait OpticResultExt<S: Shape, O: Shape, E: Shape>: Optic<S, Output = Result<O, E>> {
    /// Call [`Result::ok`].
    fn ok(self) -> Chain<Self, OperationOptic<ResultOk>> {
        self.perform(ResultOk)
    }

    /// Call [`Result::err`]
    fn err(self) -> Chain<Self, OperationOptic<ResultErr>> {
        self.perform(ResultErr)
    }
}
impl<S: Shape, O: Shape, E: Shape, Op> OpticResultExt<S, O, E> for Op where
    Op: Optic<S, Output = Result<O, E>>
{
}

/// Operation for [`OpticResultExt::ok`] that calls [`Result::ok`].
#[derive(Debug, Clone, Copy)]
pub struct ResultOk;

impl<O: Shape, E: Shape> Operation<Result<O, E>> for ResultOk {
    type Output = Option<O>;
}

impl<O, E> OperationTarget<ResultOk, Option<O>> for Result<O, E> {
    #[inline]
    fn execute(self, _op: &ResultOk) -> Option<O> {
        self.ok()
    }
}

/// Operation for [`OpticResultExt::err`] that calls [`Result::err`].
#[derive(Debug, Clone, Copy)]
pub struct ResultErr;

impl<O: Shape, E: Shape> Operation<Result<O, E>> for ResultErr {
    type Output = Option<E>;
}

impl<O, E> OperationTarget<ResultErr, Option<E>> for Result<O, E> {
    #[inline]
    fn execute(self, _op: &ResultErr) -> Option<E> {
        self.err()
    }
}

// TODO: `Option::map`
// TODO: `Result::map`
// TODO: `Result::map_err`
// TODO: flatten
// TODO: into split
// TODO: from split
// TODO: map on owned

#[cfg(test)]
mod tests {
    use super::{Lens, Optic, OpticResultExt};

    fn assert_optic<S: super::Shape, T: super::Shape>(_: impl Optic<S, Output = T>) {}

    #[test]
    fn root_works() {
        let lens = super::root::<&i32>();
        assert_optic::<&i32, &i32>(lens);

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
        assert_optic::<&String, &str>(lens);

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
        assert_optic::<&Book, &str>(lens);

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
        assert_optic::<&Book, &str>(lens);

        let mut value = Book {
            name: String::from("viv"),
        };
        assert_eq!(lens.get(&value), "viv");
        lens.get_mut(&mut value).make_ascii_uppercase();
        assert_eq!(lens.get(&value), "VIV");
    }

    #[test]
    fn project_works() {
        let optic = super::root::<&Option<i32>>().project();
        assert_optic::<&Option<i32>, Option<&i32>>(optic);

        let mut value = Some(10);
        assert_eq!(optic.get(&value), Some(&10));
        *optic
            .get_mut(&mut value)
            .expect("Given Some should return Some") = 20;
        assert_eq!(value, Some(20));
        assert_eq!(optic.get(&None), None);
    }

    #[test]
    fn result_ok() {
        let optic = super::root::<Result<&'static i32, &'static str>>().ok();
        assert_optic::<Result<&'static i32, &'static str>, Option<&'static i32>>(optic);

        let mut value = 10;
        let mut err_value = String::from("HELLO");

        assert_eq!(optic.get(Ok(&value)), Some(&10));
        assert_eq!(optic.get(Err(&err_value)), None);
        assert_eq!(optic.get_mut(Err(&mut err_value)), None);

        *optic
            .get_mut(Ok(&mut value))
            .expect("Given Ok should return Some") = 20;
        assert_eq!(value, 20);
    }

    #[test]
    fn result_err() {
        let optic = super::root::<Result<&'static i32, &'static str>>().err();
        assert_optic::<Result<&'static i32, &'static str>, Option<&'static str>>(optic);

        let mut value = 10;
        let mut err_value = String::from("HELLO");

        assert_eq!(optic.get(Err(&err_value)), Some("HELLO"));
        assert_eq!(optic.get(Ok(&value)), None);
        assert_eq!(optic.get_mut(Ok(&mut value)), None);

        optic
            .get_mut(Err(&mut err_value))
            .expect("Given Ok should return Some")
            .make_ascii_lowercase();
        assert_eq!(err_value, "hello");
    }
}
