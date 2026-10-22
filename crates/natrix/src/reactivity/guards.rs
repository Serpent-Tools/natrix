//! Implementation of guards for fine-grained reactivity with Option/Result.
#![expect(
    clippy::expect_used,
    reason = "The whole point of guards is doing unwraps internally."
)]

use crate::lens::{Chain, Optic, ProjectOptic, Projectable, Shape};
use crate::reactivity::State;
use crate::reactivity::context::RenderCtx;

/// Trait that powers the `ctx.guard` functionality, this can be implented for your own enums to
/// provide them with guard functionality.
///
/// The `S` trait paramter is the state type, while most enums can and should be generic over the
/// state we require the paramter for type checking reasons. One nice benfit of it is that it lets
/// you implement guards for external types if you specifically implment it for your state struct.
pub trait Guardable<S: 'static>: Shape {
    /// The value to be watched by `ctx.watch`, this will be given to `guard` and is expected to
    /// represent the values variant.
    type Discriminant: PartialEq + Clone + 'static;

    /// The resulting value of guarding a optic, this should usually be a version of the enum
    /// containing optics. for generic enums this is easy enough to do, basically think of this as a
    /// projection from `Optic<Output = Option<T>>` into `Option<Optic<Output = T>>`. For custom
    /// enums you might need a companion enum instead, `Optic<Output = MyEnum>` -> `MyEnumGuards`
    /// (contaning optics to fields internally.)
    type GuardResult<O>
    where
        O: Optic<&'static S, Output = Self>;

    /// Calculate the discriminant for the given value, this should correspond to the variant and
    /// should be used in `guard` to construct a optic that unwraps to the variant.
    #[must_use]
    fn discriminant(this: Self::Read<'_>) -> Self::Discriminant;

    /// Using the given discriminant wrap the given optic in a type containing optics to the inner
    /// values. For example a `Option` given `false` will return `None`, and given `true` will wrap
    /// the optic in one that unwraps the option and return the *optic* in a `Some` (i.e
    /// `Some(Optic<Output = T>)`)
    #[must_use]
    fn guard<O>(discriminant: Self::Discriminant, optic: O) -> Self::GuardResult<O>
    where
        O: Optic<&'static S, Output = Self>;
}

/// Optic that unwraps (i.e panics if `None`) a Option into the inner value.
#[derive(Debug, Copy, Clone)]
pub struct UnwrapGuard;

impl<T: Shape> Optic<Option<T>> for UnwrapGuard {
    type Output = T;

    #[inline]
    fn get<'read>(
        &self,
        value: <Option<T> as Shape>::Read<'read>,
    ) -> <Self::Output as Shape>::Read<'read> {
        value.expect("None passed to `UnwrapGuard`")
    }

    #[inline]
    fn get_mut<'write>(
        &self,
        value: <Option<T> as Shape>::Mut<'write>,
    ) -> <Self::Output as Shape>::Mut<'write> {
        value.expect("None passed to `UnwrapGuard`")
    }

    #[inline]
    fn try_get<'write>(
        &self,
        value: <Option<T> as Shape>::Mut<'write>,
    ) -> Option<<Self::Output as Shape>::Mut<'write>> {
        value
    }
}

impl<S: 'static, T: Shape + 'static> Guardable<S> for Option<T> {
    type Discriminant = bool;
    type GuardResult<O>
        = Option<Chain<O, UnwrapGuard>>
    where
        O: Optic<&'static S, Output = Self>;

    #[inline]
    fn discriminant(this: Self::Read<'_>) -> Self::Discriminant {
        this.is_some()
    }

    #[inline]
    fn guard<O>(discriminant: Self::Discriminant, optic: O) -> Self::GuardResult<O>
    where
        O: Optic<&'static S, Output = Self>,
    {
        if discriminant {
            Some(Chain(optic, UnwrapGuard))
        } else {
            None
        }
    }
}

/// Optic similar to `UnwrapGuard` that unwraps the `Ok` variant of a result.
#[derive(Debug, Copy, Clone)]
pub struct OkGuard;

/// Optic similar to `UnwrapGuard` that unwraps the `Err` variant of a result.
#[derive(Debug, Copy, Clone)]
pub struct ErrGuard;

impl<O: Shape, E: Shape> Optic<Result<O, E>> for OkGuard {
    type Output = O;

    #[inline]
    fn get<'read>(
        &self,
        value: <Result<O, E> as Shape>::Read<'read>,
    ) -> <Self::Output as Shape>::Read<'read> {
        value.ok().expect("Err passed to OkGuard")
    }

    #[inline]
    fn get_mut<'write>(
        &self,
        value: <Result<O, E> as Shape>::Mut<'write>,
    ) -> <Self::Output as Shape>::Mut<'write> {
        value.ok().expect("Err passed to OkGuard")
    }

    #[inline]
    fn try_get<'write>(
        &self,
        value: <Result<O, E> as Shape>::Mut<'write>,
    ) -> Option<<Self::Output as Shape>::Mut<'write>> {
        value.ok()
    }
}

impl<O: Shape, E: Shape> Optic<Result<O, E>> for ErrGuard {
    type Output = E;

    #[inline]
    fn get<'read>(
        &self,
        value: <Result<O, E> as Shape>::Read<'read>,
    ) -> <Self::Output as Shape>::Read<'read> {
        value.err().expect("Ok passed to ErrGuard")
    }

    #[inline]
    fn get_mut<'write>(
        &self,
        value: <Result<O, E> as Shape>::Mut<'write>,
    ) -> <Self::Output as Shape>::Mut<'write> {
        value.err().expect("Ok passed to ErrGuard")
    }

    #[inline]
    fn try_get<'write>(
        &self,
        value: <Result<O, E> as Shape>::Mut<'write>,
    ) -> Option<<Self::Output as Shape>::Mut<'write>> {
        value.err()
    }
}

impl<S: 'static, O: Shape + 'static, E: Shape + 'static> Guardable<S> for Result<O, E> {
    type Discriminant = bool;
    type GuardResult<Opt>
        = Result<Chain<Opt, OkGuard>, Chain<Opt, ErrGuard>>
    where
        Opt: Optic<&'static S, Output = Self>;

    fn discriminant(this: Self::Read<'_>) -> Self::Discriminant {
        this.is_ok()
    }

    fn guard<Opt>(discriminant: Self::Discriminant, optic: Opt) -> Self::GuardResult<Opt>
    where
        Opt: Optic<&'static S, Output = Self>,
    {
        if discriminant {
            Ok(Chain(optic, OkGuard))
        } else {
            Err(Chain(optic, ErrGuard))
        }
    }
}

impl<S, T> Guardable<S> for &'static T
where
    T: Projectable + 'static,
    T::Result: Guardable<S>,
    S: 'static,
{
    type Discriminant = <T::Result as Guardable<S>>::Discriminant;
    type GuardResult<O>
        = <T::Result as Guardable<S>>::GuardResult<Chain<O, ProjectOptic>>
    where
        O: Optic<&'static S, Output = Self>;

    #[inline]
    fn discriminant(this: Self::Read<'_>) -> Self::Discriminant {
        <T::Result as Guardable<S>>::discriminant(this.project())
    }

    #[inline]
    fn guard<O>(discriminant: Self::Discriminant, optic: O) -> Self::GuardResult<O>
    where
        O: Optic<&'static S, Output = Self>,
    {
        <T::Result as Guardable<S>>::guard(discriminant, Chain(optic, ProjectOptic))
    }
}

impl<S: State> RenderCtx<'_, '_, S> {
    /// Get a guard that can be used to retrieve the  variant of a enum without having to
    /// use `.unwrap` (or similar) in sub closures.
    /// Should be used to achieve fine-grained reactivity (internally this uses `.watch` on `.is_some()`/`.is_ok()`, etc.)
    ///
    /// # Why?
    /// The usecase can be seen by considering this logic:
    /// ```rust
    /// # use natrix::prelude::*;
    /// # #[derive(State)]
    /// # struct App {value: Signal<Option<u32>>}
    /// # fn render() -> impl Element<App> {
    /// # |ctx: RenderCtx<App>| {
    /// if let Some(value) = *ctx.value {
    ///     e::div().text(value)
    /// } else {
    ///     e::div().text("Is none")
    /// }
    /// # }}
    /// ```
    /// The issue here is that the outer div (which might be a more expensive structure to create) is
    /// recreated everytime `value` changes, even if it is `Some(0) -> Some(1)`
    /// This is where you might reach for `ctx.watch`, and in fact that works perfectly:
    /// ```rust
    /// # use natrix::prelude::*;
    /// # #[derive(State)]
    /// # struct App {value: Signal<Option<u32>>}
    /// # fn render() -> impl Element<App> {
    /// # |mut ctx: RenderCtx<App>| {
    /// if ctx.watch(|ctx| ctx.value.is_some()) {
    ///     e::div().text(|ctx: RenderCtx<App>| ctx.value.unwrap())
    /// } else {
    ///     e::div().text("Is none")
    /// }
    /// # }}
    /// ```
    /// And this works, Now a change from `Some(0)` to `Some(1)` will only run the inner closure and
    /// the outer div is reused. but there is one downside, we need `.unwrap` because the inner closure is
    /// technically isolated, and this is ugly, and its easy to do by accident. and you might forget
    /// the outer condition.
    ///
    /// This is where guards come into play:
    /// ```rust
    /// # use natrix::prelude::*;
    /// # use natrix::lens;
    /// # #[derive(State)]
    /// # struct App {value: Signal<Option<u32>>}
    /// # fn render() -> impl Element<App> {
    /// # |mut ctx: RenderCtx<App>| {
    /// if let Some(value_guard) = ctx.guard(field!(App, .value).deref()) {
    ///     e::div().text(move |mut ctx: RenderCtx<App>| *value_guard.get(&ctx))
    /// } else {
    ///     e::div().text("Is none")
    /// }
    /// # }}
    /// ```
    /// Here `value_guard` is actually not the value at all, its a lightweight `Optic` that can be
    /// captured by child closures and basically is a way to say "I know that in this context this
    /// value is `Some`"
    ///
    /// Internally this uses `ctx.watch`, and the returned optic panics if the value is `None`
    /// (which should never happen in its intended scope).
    /// Guard also functions on `Result`, and any other type that implements `Guardable`
    ///
    /// # Panics
    /// The returned optic will panic if called outside intended scope.
    pub fn guard<O, G>(&mut self, optic: O) -> G::GuardResult<O>
    where
        O: Optic<&'static S, Output = G>,
        G: Guardable<S>,
    {
        let discriminant = self.watch(move |ctx| G::discriminant(optic.get(&ctx)));
        G::guard(discriminant, optic)
    }
}
