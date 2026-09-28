//! Signals for tracking reactive dependencies and modifications.

use std::cell::RefCell;
use std::ops::{Deref, DerefMut};

use crate::access::{Downgrade, Project, Ref, RefClosure};
use crate::error_handling::log_or_panic;
use crate::prelude::State;
use crate::reactivity::core;
use crate::reactivity::core::SignalDepList;

/// A signal tracks reads and writes to a value, as well as dependencies.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Signal<T> {
    /// The data to be tracked.
    data: T,
    /// A collection of the dependencies.
    #[cfg_attr(feature = "serde", serde(skip))]
    deps: RefCell<SignalDepList>,
}

impl<T: std::fmt::Debug> std::fmt::Debug for Signal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (**self).fmt(f)
    }
}

impl<T> From<T> for Signal<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Signal<T> {
    /// Create a new signal with the specified data
    pub fn new(data: T) -> Self {
        Self {
            data,
            deps: RefCell::new(SignalDepList::new()),
        }
    }
}

impl<T: 'static> State for Signal<T> {
    fn set(&mut self, new: Self) {
        **self = new.data;
    }
}

impl<T> Deref for Signal<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        if let Some(hook) = core::statics::current_hook() {
            if let Ok(mut deps) = self.deps.try_borrow_mut() {
                deps.insert(hook);
            } else {
                log_or_panic!("Signal deps list already borrowed");
            }
        }

        &self.data
    }
}
impl<T> DerefMut for Signal<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        core::statics::reg_dirty_list(|| self.deps.get_mut().create_iter_and_clear());

        &mut self.data
    }
}

impl<T: Default> Default for Signal<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

/// Trait for `Project` type whose target contains a state.
/// Such as `Option<Signal<...>>`
pub trait ProjectIntoState: Project {}
impl<T: State> ProjectIntoState for Option<T> {}
impl<T: State, E: State> ProjectIntoState for Result<T, E> {}

/// A signal over a type that implements `Project`, such as `Option`.
/// Allows modifying the inner projected value without triggering re-renders of all values
/// subscribed to this signal.
pub struct ProjectableSignal<T: ProjectIntoState> {
    /// The data itself
    data: T,
    /// the dependency on the `T`s variant state.
    deps: RefCell<SignalDepList>,
}

impl<T> State for ProjectableSignal<T>
where
    T: ProjectIntoState + 'static,
{
    #[inline]
    fn set(&mut self, new: Self) {
        self.update(new.data);
    }
}

impl<T> ProjectableSignal<T>
where
    T: ProjectIntoState,
{
    /// Create a new projected signal.
    #[inline]
    #[must_use]
    pub fn new(data: T) -> Self {
        Self {
            data,
            deps: RefCell::new(SignalDepList::new()),
        }
    }

    /// Update the wrapping value, triggering updates of all readers.
    #[inline]
    pub fn update(&mut self, new: T) {
        self.data = new;
        core::statics::reg_dirty_list(|| self.deps.get_mut().create_iter_and_clear());
    }

    /// Convert from a `&mut ProjectableSignal<Option<T>>` into a `Option<&mut T>`
    /// (Or similarly for any other projectable value)
    /// This does *not* mark the `ProjectableSignal` as dirty.
    #[inline]
    #[must_use]
    pub fn as_mut<'this>(&'this mut self) -> <T::Projected<'this> as Downgrade<'this>>::MutOutput
    where
        T::Projected<'this>: Downgrade<'this>,
    {
        (Ref::project).call_mut(&mut self.data)
    }
}

impl<'reference, T> Ref<'reference, ProjectableSignal<T>>
where
    T: ProjectIntoState,
{
    /// Convert a `Ref<ProjectableSignal<Option<T>>>` into a `Option<Ref<T>>`,
    /// (Or similarly for any other projectable value)
    /// In the mut path this does *not* mark the `ProjectableSignal` as dirty.
    #[must_use]
    pub fn project_signal(self) -> T::Projected<'reference> {
        if let Ref::Read(this) = &self
            && let Some(hook) = core::statics::current_hook()
        {
            if let Ok(mut deps) = this.deps.try_borrow_mut() {
                deps.insert(hook);
            } else {
                log_or_panic!("Deps list overlapping borrow");
            }
        }
        crate::field!((self).data).project()
    }
}

impl<T> Deref for ProjectableSignal<T>
where
    T: ProjectIntoState,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        if let Some(hook) = core::statics::current_hook() {
            if let Ok(mut deps) = self.deps.try_borrow_mut() {
                deps.insert(hook);
            } else {
                log_or_panic!("Signal deps list already borrowed");
            }
        }

        &self.data
    }
}

impl<T> Default for ProjectableSignal<T>
where
    T: Default + ProjectIntoState,
{
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

#[cfg(test)]
#[expect(clippy::expect_used, reason = "tests")]
mod tests {

    use std::collections::HashSet;

    use super::*;
    use crate::reactivity::core::{HookKey, statics};

    #[test]
    fn reading_signals_makes_them_appear_in_dirty() {
        let mut foo = Signal::new(0);
        let mut bar = Signal::new(0);

        let hook = HookKey::new(0, 0);

        statics::with_hook(hook, || {
            let _ = *foo;
            let _ = *bar;
        });

        let (dirty, ()) = statics::with_dirty_tracking(|| {
            *foo = 10;
            *bar = 20;
        });

        let mut dirty = dirty.into_iter();
        let mut first = dirty.next().expect("Expected at least one element");
        let mut second = dirty.next().expect("Expected at least two elements");
        assert!(dirty.next().is_none());

        assert_eq!(first.next(), Some(hook));
        assert_eq!(first.next(), None);

        assert_eq!(second.next(), Some(hook));
        assert_eq!(second.next(), None);
    }

    #[test]
    fn projectable_signal_modify_outer_alerts_both() {
        let mut signal = ProjectableSignal::new(Some(Signal::new(10)));
        let hook_outer = HookKey::new(0, 0);
        let hook_inner = HookKey::new(1, 0);

        statics::with_hook(hook_outer, || {
            let _ = *signal;
        });
        statics::with_hook(hook_inner, || {
            if let Some(inner) = &*signal {
                let _: i32 = **inner;
            }
        });

        let (dirty, ()) = statics::with_dirty_tracking(|| {
            signal.update(None);
        });

        let hooks: HashSet<_> = dirty.into_iter().flatten().collect();
        assert_eq!(hooks, HashSet::from([hook_outer, hook_inner]));
    }

    #[test]
    fn projectable_signal_modify_inner_alerts_on() {
        let mut signal = ProjectableSignal::new(Some(Signal::new(10)));
        let hook_outer = HookKey::new(0, 0);
        let hook_inner = HookKey::new(1, 0);

        statics::with_hook(hook_outer, || {
            let _ = *signal;
        });
        statics::with_hook(hook_inner, || {
            if let Some(inner) = &*signal {
                let _: i32 = **inner;
            }
        });

        let (dirty, ()) = statics::with_dirty_tracking(|| {
            if let Some(inner) = signal.as_mut() {
                **inner = 10;
            }
        });

        let hooks: HashSet<_> = dirty.into_iter().flatten().collect();
        assert_eq!(hooks, HashSet::from([hook_inner]));
    }
}
