//! Various async versions of js callback apis
#![cfg(feature = "async_utils")]
use std::time::Duration;

use futures_channel::oneshot;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::Closure;
use web_sys::js_sys::Function;

use crate::error_handling::log_or_panic;

/// A guard that executes a callback when dropped.
///
/// This is primarily used to cleanup js resources when stuff like a Future is dropped.
pub(crate) struct DropGuard<F>
where
    F: FnOnce(),
{
    /// The callback to be executed on drop.
    callback: Option<F>,
}

impl<F> DropGuard<F>
where
    F: FnOnce(),
{
    /// Creates a new guard that will call the provided function on drop.
    pub(crate) fn new(callback: F) -> Self {
        Self {
            callback: Some(callback),
        }
    }

    /// Disables the callback, preventing it from being called on drop.
    pub(crate) fn cancel(&mut self) {
        self.callback = None;
    }
}

impl<F> Drop for DropGuard<F>
where
    F: FnOnce(),
{
    fn drop(&mut self) {
        if let Some(callback) = self.callback.take() {
            callback();
        }
    }
}

/// Sleeps for the given duration using js `setTimeout`.
///
/// Returns `None` if the given duration overflows a i32 milliseconds (~24 days).
/// Reasonably this is a rare condition to hit for sane code, hence you can use `sleep_milliseconds`
/// and pass in a i32 of milliseconds directly.
#[must_use]
pub fn sleep(duration: Duration) -> Option<impl Future<Output = ()>> {
    let milis = duration.as_millis().try_into().ok()?;
    Some(sleep_milliseconds(milis))
}

/// A helper function to wait for an asynchronous browser event with cancellation support.
///
/// This function encapsulates the common logic for setting up a oneshot channel,
/// creating a JS closure, calling a setup function (like `setTimeout` or `requestAnimationFrame`),
/// and managing cancellation with a `DropGuard`.
async fn wait_with_cancellation<T: Copy + 'static>(
    setup: impl FnOnce(&Function) -> Result<T, JsValue>,
    cancel: impl FnOnce(T) + 'static,
    setup_err_msg: &str,
    recv_err_msg: &str,
) {
    let (tx, rx) = oneshot::channel();
    let closure = Closure::once(move || {
        let _ = tx.send(());
    });
    let function: Function = closure.as_ref().clone().into();

    let Ok(id) = setup(&function) else {
        log_or_panic!("{}", setup_err_msg);
        return;
    };

    let mut drop_guard = DropGuard::new(move || {
        cancel(id);
    });

    if rx.await.is_err() {
        log_or_panic!("{}", recv_err_msg);
        return;
    }

    drop_guard.cancel();
}

/// Sleeps for the given milliseconds using js `setTimeout`.
///
/// # Why `i32`?
/// Because thats what the `web_sys` `setTimeout` binding uses.
pub async fn sleep_milliseconds(milis: i32) {
    wait_with_cancellation(
        |function| {
            crate::get_window()
                .set_timeout_with_callback_and_timeout_and_arguments_0(function, milis)
        },
        move |timeout_id| {
            crate::get_window().clear_timeout_with_handle(timeout_id);
        },
        "Failed to set timeout. This is a bug in the browser or the framework.",
        "Failed to receive timeout signal",
    )
    .await;
}

/// Wait for the next browser animation frame using `requestAnimationFrame`.
pub async fn next_animation_frame() {
    wait_with_cancellation(
        |function| crate::get_window().request_animation_frame(function),
        move |frame_id| {
            let res = crate::get_window().cancel_animation_frame(frame_id);
            if res.is_err() {
                log_or_panic!(
                    "Failed to cancel animation frame. This is a bug in the browser or the framework."
                );
            }
        },
        "Failed to request animation frame. This is a bug in the browser or the framework.",
        "Failed to receive animation frame signal",
    )
    .await;
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn drop_guard_basic_functionality() {
        let called = Cell::new(false);

        {
            let _guard = DropGuard::new(|| called.set(true));
            assert!(!called.get()); // Not called yet
        } // guard drops here

        assert!(called.get()); // Called after drop
    }

    #[test]
    fn drop_guard_cancel() {
        let called = Cell::new(false);

        {
            let mut guard = DropGuard::new(|| called.set(true));
            guard.cancel();
        } // guard drops here, but callback was canceled

        assert!(!called.get());
    }

    #[test]
    fn multiple_drop_guards() {
        let counter = Cell::new(0);

        {
            let _guard1 = DropGuard::new(|| counter.set(counter.get() + 1));
            let _guard2 = DropGuard::new(|| counter.set(counter.get() + 2));
            let _guard3 = DropGuard::new(|| counter.set(counter.get() + 3));

            assert_eq!(counter.get(), 0);
        } // Guards drop in reverse order (LIFO)

        assert_eq!(counter.get(), 6); // 3 + 2 + 1
    }

    #[test]
    #[should_panic(expected = "Callback panic")]
    #[expect(clippy::panic, reason = "Its a test")]
    fn drop_guard_panicking_callback() {
        {
            let _guard = DropGuard::new(|| panic!("Callback panic"));
        } // guard drops here and should panic
    }

    #[test]
    fn drop_guard_with_captured_values() {
        let mut value = String::from("initial");

        {
            let _guard = DropGuard::new(|| {
                value = String::from("modified");
            });
        }

        assert_eq!(value, "modified");
    }

    #[test]
    fn nested_drop_guards() {
        let counter = Cell::new(0);

        {
            let _outer = DropGuard::new(|| {
                counter.set(counter.get() + 1);

                let _inner = DropGuard::new(|| {
                    counter.set(counter.get() + 10);
                });
            });
        }

        assert_eq!(counter.get(), 11); // 1 + 10
    }
}
