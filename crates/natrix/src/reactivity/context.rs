//! Context types for accessing reactive state.
//!
//! This module defines the core context types that provide access to application state
//! during different phases of execution:
//! - `InnerCtx<S>`: The internal framework context (not directly exposed to users)
//! - `RenderCtx<S>`: Read-only context for rendering
//! - `EventCtx<S>`: Mutable context for event handlers

use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::{Rc, Weak};

use crate::error_handling::log_or_panic;
use crate::reactivity::State;
use crate::reactivity::core::{HookStore, RenderingState};

/// The core framework context, holds user state and reactive infrastructure.
pub(crate) struct InnerCtx<S: State> {
    /// The user-defined reactive state
    pub(super) data: S,
    /// A weak reference to ourself for event handlers to access without explicit passing
    pub(crate) this: Weak<RefCell<Self>>,
    /// Reactive hooks storage
    pub(super) hooks: HookStore<S>,
}

impl<S: State> InnerCtx<S> {
    /// Create a minimal instance without wrapping in Rc
    ///
    /// Warning: the `this` weak reference is not set up yet
    pub(super) fn create_base(data: S) -> Self {
        Self {
            data,
            this: Weak::new(),
            hooks: HookStore::new(),
        }
    }

    /// Convert this into a finalized state by populating the weak reference and returning an Rc
    pub(super) fn finalize(self) -> Rc<RefCell<Self>> {
        let this = Rc::new(RefCell::new(self));

        if let Ok(mut borrow) = this.try_borrow_mut() {
            borrow.this = Rc::downgrade(&this);
        } else {
            log_or_panic!("State (somehow) already borrowed in `finalize`");
        }

        this
    }

    /// Create a new instance of the state, returning an `Rc` to it
    pub(super) fn new(data: S) -> Rc<RefCell<Self>> {
        Self::create_base(data).finalize()
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
