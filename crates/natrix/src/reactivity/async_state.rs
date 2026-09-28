//! Async context implementation for use with futures.

use std::cell::RefCell;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::rc::Weak;

use crate::error_handling::log_or_panic;
use crate::reactivity::context::InnerCtx;
use crate::reactivity::{EventCtx, State};

impl<S: State> EventCtx<'_, S> {
    /// Spawn an async task in the local event loop, which will run on the next possible moment.
    pub fn use_async<C, F>(&self, func: C)
    where
        C: FnOnce(AsyncCtxHandle<S>) -> F,
        F: Future<Output = Option<()>> + 'static,
    {
        let handle = AsyncCtxHandle {
            inner: Weak::clone(&self.0.this),
        };
        let future = func(handle);
        let future = async {
            let _ = future.await;
        };

        let future = PanicCheckFuture { inner: future };

        wasm_bindgen_futures::spawn_local(future);
    }
}

/// A wrapper future that checks `has_panicked` before resolving.
///
/// If you are using `wasm_bindgen_futures` directly you should wrap your futures in this.
#[pin_project::pin_project]
pub struct PanicCheckFuture<F> {
    /// The future to run
    #[pin]
    pub inner: F,
}

impl<F: Future> Future for PanicCheckFuture<F> {
    type Output = F::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if crate::panics::has_panicked() {
            std::task::Poll::Pending
        } else {
            self.project().inner.poll(cx)
        }
    }
}

/// Context for async operations, providing mutable access to state.
pub struct AsyncCtx<'state, S: State>(&'state mut InnerCtx<S>);

impl<S: State> Deref for AsyncCtx<'_, S> {
    type Target = S;
    fn deref(&self) -> &Self::Target {
        &self.0.data
    }
}

impl<S: State> DerefMut for AsyncCtx<'_, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0.data
    }
}

/// A handle to access state from async contexts.
///
/// Combines a `Weak<RefCell<InnerCtx<S>>>` with safe borrowing operations
/// to prevent borrow errors in async code.
#[must_use]
pub struct AsyncCtxHandle<S: State> {
    /// The weak reference to the context
    inner: Weak<RefCell<InnerCtx<S>>>,
}

impl<S: State> Clone for AsyncCtxHandle<S> {
    fn clone(&self) -> Self {
        Self {
            inner: Weak::clone(&self.inner),
        }
    }
}

impl<S: State> AsyncCtxHandle<S> {
    /// Run a function on the state, returning `None` if the element was dropped.
    ///
    /// # Reactivity
    /// Modifications to state are tracked and trigger UI updates when the closure exits.
    #[must_use]
    pub fn update<R>(&self, func: impl FnOnce(AsyncCtx<S>) -> R) -> Option<R> {
        let rc = self.inner.upgrade()?;
        let Ok(mut borrow) = rc.try_borrow_mut() else {
            log_or_panic!("State borrowed while already borrowed.");
            return None;
        };

        let result = borrow.track_changes(|ctx| func(AsyncCtx(ctx)));
        Some(result)
    }
}
