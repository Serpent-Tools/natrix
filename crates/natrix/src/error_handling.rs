//! Various internal error handling mechanisms

/// Cold path hint, causes compiler to better optimize unlikely error paths.
#[cold]
pub(crate) fn cold_path() {}

/// Panic on `Err` value in debug mode.
macro_rules! log_or_panic_result {
    ($expr:expr, $($msg:expr),*) => {
        let res = $expr;
        match res {
            Ok(_) => {}
            Err(_) => {
                $crate::error_handling::log_or_panic!($($msg),*);
            }
        }
    };
}

/// Version of stdlib `debug_assert` that uses `log_or_panic` in order to get logging.
macro_rules! log_or_panic_assert {
    ($check:expr, $($msg:expr),*) => {
        if !$check {
            $crate::error_handling::log_or_panic!($($msg),*);
        }
    };
}

/// Panic on debug builds only
///
/// This lives in a gray zone between normal panics and Result/Option.
/// It should be preferred over `panic!` in basically every case, but whether its better than
/// `Result` or `Option` comes down to what a caller can reasonably be expected to do in the error
/// case.
///
/// For example, most uses of this is in framework internals, where the entire callstack is pure
/// natrix code. there is no library user to make a decision. so if we used `Result`/`Option` we
/// would just push the "ignore it" decision to the top of the stack.
/// instead panicking early gives a better pointer to the error location, as well as allows for
/// easier recovery paths in release builds.
///
/// This should only be used when the error path is the result of a bug in the framework or user
/// code, *not for errors that can reasonably happen because of end user input*. For example if say
/// `replaceChild` call fails thats very unexpected and we flag it in debug builds, because likely
/// the developer did some strange dom modification outside of the framework.
/// But in release, well we can survive missing a dom update.
///
/// TL;DR: Use this when an error both indicates a serious bug in framework or user code, and there
/// is a reasonable way to recover from it in release builds.
///
/// *IMPORTANT:* Using this for user bugs need to be carefully thought about, failing to check
/// user inputs for validity is *not* a bug this should catch, and should use `Option`/`Result`
/// instead. This should only be used for stuff that is 100% a framework bug, or user doing
/// something very cursed.
macro_rules! log_or_panic {
    ($($msg:expr),*) => {
        $crate::error_handling::cold_path();


        ::log::error!($($msg),*);
        if cfg!(debug_assertions) {
            panic!($($msg),*);

        }
    };
}

pub(crate) use {log_or_panic, log_or_panic_assert, log_or_panic_result};

#[cfg(test)]
mod tests {
    #[test]
    #[should_panic(expected = "Error in release mode")]
    fn test_debug_expect() {
        log_or_panic_result!(Err::<(), _>("error"), "Error in release mode");
    }

    #[test]
    #[should_panic(expected = "This won't panic in release")]
    fn test_debug_panic() {
        log_or_panic!("This won't panic in release");
    }
}
