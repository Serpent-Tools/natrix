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

impl<T: ProjectIntoState> Signal<T> {
    /// Project a `&mut Signal<Option<T>>` (or other `Project` type) into a `Option<&mut T>`,
    /// without triggering re-renders of subscribers to this `Signal`.
    /// This is mainly useful with your own state types.
    ///
    /// Consider the following code:
    /// ```
    /// # use natrix::prelude::*;
    /// #[derive(State)]
    /// struct Book {
    ///     name: Signal<String>,
    ///     cost: Signal<u8>,
    /// }
    ///
    /// #[derive(State)]
    /// struct App {
    ///     book: Signal<Option<Book>>
    /// }
    ///
    /// fn modify_book(app: &mut App) {
    ///     if let Some(book) = (*app.book).as_mut() {
    ///         *book.cost += 2;
    ///     }
    /// }
    /// ```
    /// Here because we dereference the `app.book` signal any closures dependent on `name` (and by
    /// extension the whole book) would re-run even though we only touch cost.
    /// If we instead use this method:
    /// ```
    /// # use natrix::prelude::*;
    /// # #[derive(State)]
    /// # struct Book {
    /// #     name: Signal<String>,
    /// #    cost: Signal<u8>,
    /// # }
    /// #
    /// # #[derive(State)]
    /// # struct App {
    /// #     book: Signal<Option<Book>>
    /// # }
    /// #
    /// fn modify_book(app: &mut App) {
    ///     if let Some(book) = app.book.project_mut() {
    ///         *book.cost += 2;
    ///     }
    /// }
    /// ```
    /// Now only dependencies of `book.cost` will re-run.
    ///
    /// This method requires that the signal type (in addition to the normal `Project`) implements
    /// `ProjectIntoState`, which asserts that the project target is also a `State` type (sadly rust
    /// does not give a way to easily express this restriction against just `Project`)
    ///
    /// The [`Project`] implementation on `Signal` behaves the same way in its mut path.
    #[must_use]
    #[inline]
    pub fn project_mut<'this>(
        &'this mut self,
    ) -> <T::Projected<'this> as Downgrade<'this>>::MutOutput
    where
        T::Projected<'this>: Downgrade<'this>,
    {
        (Ref::project).call_mut(&mut self.data)
    }

    /// Project into the inner value, without triggering re-renders of subscribers to this `Signal`.
    ///
    /// Alias for [`Self::project_mut`].
    #[must_use]
    #[inline]
    pub fn as_mut<'this>(&'this mut self) -> <T::Projected<'this> as Downgrade<'this>>::MutOutput
    where
        T::Projected<'this>: Downgrade<'this>,
    {
        self.project_mut()
    }
}

impl<T: ProjectIntoState> Project for Signal<T> {
    type Projected<'reference>
        = T::Projected<'reference>
    where
        Self: 'reference;

    /// The mut path leaves subscribers to the whole value alone, like [`Signal::project_mut`].
    fn project(value: Ref<'_, Self>) -> Self::Projected<'_> {
        if let Ref::Read(this) = &value
            && let Some(hook) = core::statics::current_hook()
        {
            if let Ok(mut deps) = this.deps.try_borrow_mut() {
                deps.insert(hook);
            } else {
                log_or_panic!("Signal deps list already borrowed");
            }
        }

        crate::field!(value.data).project()
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
    fn modify_outer_signal_alerts_both() {
        let mut signal = Signal::new(Some(Signal::new(10)));
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
            *signal = None;
        });

        let hooks: HashSet<_> = dirty.into_iter().flatten().collect();
        assert_eq!(hooks, HashSet::from([hook_outer, hook_inner]));
    }

    #[test]
    fn project_mut_alerts_only_inner() {
        let mut signal = Signal::new(Some(Signal::new(10)));
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
            if let Some(inner) = signal.project_mut() {
                **inner = 10;
            }
        });

        let hooks: HashSet<_> = dirty.into_iter().flatten().collect();
        assert_eq!(hooks, HashSet::from([hook_inner]));
    }

    #[test]
    fn as_mut_resolves_to_signal_not_option() {
        let mut signal = Signal::new(Some(Signal::new(10)));
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
