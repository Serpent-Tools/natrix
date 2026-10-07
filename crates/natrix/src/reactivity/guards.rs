//! Implementation of guards for fine-grained reactivity with Option/Result.
#![expect(
    clippy::unreachable,
    reason = "The whole point of guards is doing unwraps internally."
)]

use crate::access::Ref;
use crate::reactivity::State;
use crate::reactivity::context::RenderCtx;

impl<S: State> RenderCtx<'_, '_, S> {
    /// Get a guard getter that can be used to retrieve the `Some` variant of a option without having to
    /// use `.unwrap`.
    /// Should be used to achieve fine-grained reactivity (internally this uses `.watch` on `.is_some()`)
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
    /// # #[derive(State)]
    /// # struct App {value: Signal<Option<u32>>}
    /// # fn render() -> impl Element<App> {
    /// # |mut ctx: RenderCtx<App>| {
    /// if let Some(value_guard) = ctx.guard_option(|ctx| field!(ctx.value).deref().project()) {
    ///     e::div().text(move |mut ctx: RenderCtx<App>| *value_guard.call_read(&ctx))
    /// } else {
    ///     e::div().text("Is none")
    /// }
    /// # }}
    /// ```
    /// Here `value_guard` is actually not the value at all, its a lightweight value that can be
    /// captured by child closures and basically is a way to say "I know that in this context this
    /// value is `Some`"
    ///
    /// Internally this uses `ctx.watch`, and the returned getter panics if the value is `None`
    /// (which should never happen in its intended scope).
    /// Guard also functions on `Result`
    ///
    /// # Panics
    /// The returned getter will panic if called outside intended scope,
    /// unless called with `.call_failable`, in which case it returns `None`.
    #[inline]
    pub fn guard_option<F, T>(
        &mut self,
        getter: F,
    ) -> Option<impl Fn(Ref<S>) -> Ref<T> + Clone + use<F, T, S>>
    where
        F: Fn(Ref<S>) -> Option<Ref<T>> + Clone + 'static,
    {
        let watch_getter = getter.clone();
        let check = self.watch(move |render| watch_getter(Ref::Read(&render.ctx.data)).is_some());
        if check {
            Some(create_getter(move |ctx| getter(ctx)))
        } else {
            None
        }
    }

    /// Same as `guard_option`, but for `Result`
    #[expect(clippy::missing_errors_doc, reason = "This is transforming a Result")]
    #[inline]
    pub fn guard_result<F, T, E>(
        &mut self,
        getter: F,
    ) -> Result<
        // Once rust gets better about `use` bounds we can get rid of `T`/`E` in the different
        // bounds.
        impl Fn(Ref<S>) -> Ref<T> + Clone + use<F, T, E, S>,
        impl Fn(Ref<S>) -> Ref<E> + Clone + use<F, T, E, S>,
    >
    where
        F: Fn(Ref<S>) -> Result<Ref<T>, Ref<E>> + Clone + 'static,
        T: 'static,
        E: 'static,
    {
        let watch_getter = getter.clone();
        let check = self.watch(move |render| watch_getter(Ref::Read(&render.ctx.data)).is_ok());

        if check {
            Ok(create_getter(move |ctx| getter(ctx).ok()))
        } else {
            Err(create_getter(move |ctx| getter(ctx).err()))
        }
    }
}

/// Create a getter that handles the logic of guard getters.
#[inline]
fn create_getter<S, R>(
    getter: impl Fn(Ref<S>) -> Option<Ref<R>> + Clone,
) -> impl Fn(Ref<S>) -> Ref<R> + Clone {
    #[inline]
    move |ctx| {
        let is_failable = matches!(ctx, Ref::FaillableMut(_));
        match getter(ctx) {
            None => {
                if is_failable {
                    Ref::FaillableMut(None)
                } else {
                    log::error!("Guard closure got `None` in non_failable mode.");
                    unreachable!("Guard closure got `None` in non_failable mode.");
                }
            }
            Some(value) => value,
        }
    }
}
