//! Implementation of `ctx.watch` for fine-grained reactivity.

use crate::error_handling::log_or_panic;
use crate::reactivity::context::{InnerCtx, RenderCtx};
use crate::reactivity::core::{HookKey, ReactiveHook, RenderingState, UpdateResult, statics};
use crate::reactivity::{KeepAlive, State};

/// The watcher hook / signal
struct WatchState<F, T> {
    /// Function to calculate the state
    calc_value: F,
    /// The previous cached value
    last_value: T,
    /// The dependency that owns us
    dep: HookKey,
    /// Keep alive objects
    keep_alive: Vec<KeepAlive>,
    /// Child hooks
    hooks: Vec<HookKey>,
}

impl<S, F, T> ReactiveHook<S> for WatchState<F, T>
where
    S: State,
    T: PartialEq,
    F: Fn(RenderCtx<S>) -> T,
{
    fn update(&mut self, ctx: &mut InnerCtx<S>, you: HookKey) -> UpdateResult {
        self.keep_alive.clear();
        let hooks = std::mem::take(&mut self.hooks);

        let new_value = ctx.track_reads(you, |ctx| {
            let render = RenderCtx {
                ctx,
                render_state: RenderingState {
                    keep_alive: &mut self.keep_alive,
                    hooks: &mut self.hooks,
                },
            };
            (self.calc_value)(render)
        });

        if new_value == self.last_value {
            UpdateResult::DropHooks(hooks)
        } else {
            UpdateResult::RunHook(self.dep, hooks)
        }
    }

    fn drop_us(self: Box<Self>) -> Vec<HookKey> {
        Vec::new()
    }
}

impl<S: State> RenderCtx<'_, '_, S> {
    /// Calculate the value using the function and cache it using `clone`.
    /// Then whenever any signals read in the function are modified, re-run the function and check
    /// if the new result is different.
    /// Only reruns the caller when the item is different.
    ///
    /// # Example
    /// ```rust
    /// # use natrix::prelude::*;
    /// # #[derive(State)]
    /// # struct App {value: Signal<u32>}
    /// #
    /// # fn render() -> impl Element<App> {
    /// # |mut ctx: RenderCtx<App>| {
    /// if ctx.watch(|ctx| *ctx.value > 2) {
    ///     e::div().text(|ctx: RenderCtx<App>| *ctx.value)
    /// } else {
    ///     e::div().text("Value is too low")
    /// }
    /// # }}
    /// ```
    #[inline]
    pub fn watch<T, F>(&mut self, func: F) -> T
    where
        F: for<'context, 'state> Fn(RenderCtx<'context, 'state, S>) -> T + 'static,
        T: PartialEq + Clone + 'static,
    {
        let me = self.ctx.hooks.reserve_key();
        let mut hooks = Vec::new();
        let mut keep_alive = Vec::new();

        let keep_alive_borrow = &mut keep_alive;
        let hooks_borrow = &mut hooks;

        let result = self.ctx.track_reads(me, |ctx| {
            let render = RenderCtx {
                ctx,
                render_state: RenderingState {
                    keep_alive: keep_alive_borrow,
                    hooks: hooks_borrow,
                },
            };
            func(render)
        });

        let Some(dep) = statics::current_hook() else {
            log_or_panic!("`ctx.watch` called from outside a hook");
            return result;
        };
        let hook = WatchState {
            calc_value: Box::new(func),
            last_value: result.clone(),
            dep,
            keep_alive,
            hooks,
        };
        self.ctx.hooks.set_hook(me, Box::new(hook));
        self.render_state.hooks.push(me);

        result
    }
}
