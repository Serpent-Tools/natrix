//! Reactivity system for tracking dependencies and updates.
//!
//! The reactivity system is deeply integrated throughout the framework to enable
//! fine-grained updates. Key concepts:
//!
//! - **`Signal<T>`**: Reactive data primitives that track reads/writes
//! - **`State` trait**: Implemented by types that can be used as reactive state
//! - **Context types**: `InnerCtx`, `RenderCtx`, `EventCtx`, `AsyncCtx` for different execution phases
//! - **Reactive hooks**: Track dependencies and trigger targeted DOM updates
//!
//! ## Architecture
//!
//! The system has intentional cross-cutting concerns:
//! - DOM elements, attributes, and classes become reactive through the closure `WebValue`
//!   implementations (in `dom/`)
//! - Reactive hooks for DOM updates live in `dom_hooks.rs`
//! - The scheduler and hook storage live in `core.rs`
//! - Higher-level features like `watch` and `guards` build on the core primitives

pub mod context;
pub mod core;
pub mod dom_hooks;
pub mod guards;
pub mod mount;
pub mod signal;
pub mod watch;

#[cfg(feature = "async")]
pub mod async_state;

#[cfg(feature = "async")]
pub use self::async_state::{AsyncCtx, AsyncCtxHandle};
pub use self::context::{EventCtx, RenderCtx};

/// Trait automatically implemented on reactive structs by the `#[derive(State)]` macro.
pub trait State: Sized + 'static {
    /// Overwrite the value of this state while preserving reactive tracking.
    /// Generally prefer using mutable dereferences on signals instead.
    fn set(&mut self, new: Self);
}

impl State for () {
    fn set(&mut self, _new: Self) {}
}

/// Type for keeping specific objects alive in memory such as `Closure` and `Rc`
pub(crate) type KeepAlive = Box<dyn std::any::Any>;
