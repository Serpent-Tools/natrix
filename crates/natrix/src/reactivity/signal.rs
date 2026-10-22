//! Signals for tracking reactive dependencies and modifications.

use std::cell::RefCell;
use std::ops::{Deref, DerefMut};

use crate::error_handling::log_or_panic;
use crate::lens::{Projectable, Shape};
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

/// Trait for `Projectable` types whose target contains a state.
/// Such as `Option<Signal<...>>`
pub trait ProjectIntoState: Projectable {}
impl<T: State> ProjectIntoState for Option<T> {}
impl<T: State, E: State> ProjectIntoState for Result<T, E> {}

impl<T: ProjectIntoState> Signal<T> {
    /// Alias for `project_mut` for intentionally shadowing `as_mut` on `T`.
    ///
    /// Prefer using `project_mut` instead.
    #[inline]
    #[expect(
        clippy::deprecated_attributes_without_since,
        reason = "This shadows `as_mut` on `T` rather than being deprecated by a natrix release."
    )]
    #[deprecated(note = "Prefer using `project_mut` instead.")]
    pub fn as_mut(&mut self) -> <T::Result as Shape>::Mut<'_> {
        self.project_mut()
    }
}

impl<T: ProjectIntoState> Projectable for Signal<T> {
    type Result = T::Result;

    /// Project a `&mut Signal<Option<T>>` (or other `Projectable` type) into an `Option<&mut T>`,
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
    /// This method requires that the signal type (in addition to the normal `Projectable`)
    /// implements `ProjectIntoState`, which asserts that the project target is also a `State` type
    /// (sadly rust does not give a way to easily express this restriction against just
    /// `Projectable`)
    #[inline]
    fn project_mut(&mut self) -> <T::Result as Shape>::Mut<'_> {
        self.data.project_mut()
    }

    #[inline]
    fn project(&self) -> <Self::Result as Shape>::Read<'_> {
        if let Some(hook) = core::statics::current_hook() {
            if let Ok(mut deps) = self.deps.try_borrow_mut() {
                deps.insert(hook);
            } else {
                log_or_panic!("Signal deps list already borrowed");
            }
        }

        self.data.project()
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

    #[expect(
        deprecated,
        reason = "We are testing the resolution of said deprecated method."
    )]
    #[test]
    fn project_mut_resolves_to_signal_not_option() {
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
