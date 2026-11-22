//! Context types for accessing reactive state.
//!
//! This module defines the core context types that provide access to application state
//! during different phases of execution:
//! - `InnerCtx<S>`: The internal framework context (not directly exposed to users)
//! - `RenderCtx<S>`: Read-only context for rendering
//! - `EventCtx<S>`: Mutable context for event handlers

use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::Weak;

use crate::error_handling::log_or_panic;
use crate::reactivity::State;
use crate::reactivity::core::{HookStore, RenderingState};

/// A function deferred for execution when a borrow can be obtained
type DeferredFunction<S> = Box<dyn FnOnce(&mut InnerCtx<S>) + 'static>;

/// A handle to the `InnerCtx` that handles deferring events in the case of borrow collisions
pub(crate) struct CtxHandle<S: State> {
    /// The inner ctx
    ctx: RefCell<InnerCtx<S>>,
    /// The list of functions to call once a borrow is obtained
    deferred_events: RefCell<Vec<DeferredFunction<S>>>,
}

impl<S: State> CtxHandle<S> {
    /// Create a new ctx handle from the given `ctx`
    pub(crate) fn new(ctx: InnerCtx<S>) -> Self {
        Self {
            ctx: RefCell::new(ctx),
            deferred_events: RefCell::default(),
        }
    }

    /// Drain the deferred list and call them with the given ctx borrow
    fn drain_deferrede_list(&self, ctx: &mut InnerCtx<S>) {
        loop {
            let deferred_events = {
                let Ok(mut deferred_events) = self.deferred_events.try_borrow_mut() else {
                    log_or_panic!("Deferred events list already borrowed");
                    return;
                };
                std::mem::take(&mut *deferred_events)
            };

            if deferred_events.is_empty() {
                break;
            }

            for deferred in deferred_events {
                log::debug!("Calling deferred event.");
                deferred(ctx);
            }
        }
    }

    /// Run the given function with the inner ctx immediately, returning `None` if the ctx is
    /// already borrowed.
    ///
    /// Also calls deferred functions.
    #[inline]
    #[cfg(feature = "async")]
    pub(crate) fn with_immediate_state<R>(
        &self,
        func: impl FnOnce(&mut InnerCtx<S>) -> R,
    ) -> Option<R> {
        if let Ok(mut ctx) = self.ctx.try_borrow_mut() {
            let result = func(&mut ctx);
            self.drain_deferrede_list(&mut ctx);
            Some(result)
        } else {
            None
        }
    }

    /// Run the given function as soon as possible, this will happen immediately inline if the ctx
    /// isnt borrowed (most common case), otherwise it will be called as soon as the outermost frame
    /// returns to this function.
    #[inline]
    pub(crate) fn with_state(&self, func: impl FnOnce(&mut InnerCtx<S>) + 'static) {
        if let Ok(mut ctx) = self.ctx.try_borrow_mut() {
            func(&mut ctx);
            self.drain_deferrede_list(&mut ctx);
        } else {
            log::debug!("Re-entry dected, deferring event.");
            let Ok(mut deferred_events) = self.deferred_events.try_borrow_mut() else {
                log_or_panic!("Deferred events list already borrowed");
                return;
            };

            deferred_events.push(Box::new(func));
        }
    }
}

/// The core framework context, holds user state and reactive infrastructure.
pub(crate) struct InnerCtx<S: State> {
    /// The user-defined reactive state
    pub(super) data: S,
    /// A weak reference to ourself for event handlers to access without explicit passing
    pub(crate) this: Weak<CtxHandle<S>>,
    /// Reactive hooks storage
    pub(super) hooks: HookStore<S>,
}

impl<S: State> InnerCtx<S> {
    /// Create a new instance of the state, returning an `Rc` to it
    pub(super) fn new(data: S, this: Weak<CtxHandle<S>>) -> Self {
        Self {
            data,
            this,
            hooks: HookStore::new(),
        }
    }
}

/// Context for rendering, providing read-only access to user state.
///
/// This holds a mutable context internally to facilitate rendering features such as `.watch`,
/// but only exposes read-only access to the user state through `Deref`.
pub struct RenderCtx<'context, 'state, S: State> {
    /// The inner context
    pub(super) ctx: &'context mut InnerCtx<S>,
    /// The render state for tracking hooks and keep-alive objects
    pub(super) render_state: RenderingState<'state>,
}

impl<S: State> Deref for RenderCtx<'_, '_, S> {
    type Target = S;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.ctx.data
    }
}

/// Context for event handlers, providing mutable access to state.
pub struct EventCtx<'context, S: State>(pub(crate) &'context mut InnerCtx<S>);

impl<S: State> Deref for EventCtx<'_, S> {
    type Target = S;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0.data
    }
}

impl<S: State> DerefMut for EventCtx<'_, S> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0.data
    }
}
