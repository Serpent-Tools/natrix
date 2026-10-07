//! utilities for writing unit tests on wasm
#![cfg(feature = "test_utils")]
#![expect(clippy::expect_used, reason = "tests only")]

use std::cell::Cell;

use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use crate::prelude::State;
use crate::reactivity::mount::render_state;
use crate::reactivity::{KeepAlive, core};
use crate::{Element, get_document};

/// The parent of the testing env
const MOUNT_PARENT: &str = "__TESTING_PARENT";
/// The var where you should mount your element
/// This is auto created and cleaned up by `setup`
pub const MOUNT_POINT: &str = "__TESTING_MOUNT_POINT";

thread_local! {
    /// Keeps the currently mounted test tree alive.
    static CURRENT_TREE: Cell<KeepAlive> = Cell::new(Box::new(()));
}

/// Has a logger been initialized?
static LOGGER_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A simple  `log` logger that just prints to `console.log` for all levels.
// NOTE: This is used for two reasons.
// 1. the `console_log` crate color coding just adds noise in the wasm_bindgen_test capture (it
//    literally prints out the css)
// 2. This outputs everything to just `.log` for a reason, as wasm_bindgen_test splits logs per
//    level (for some reason)
struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn flush(&self) {}
    fn log(&self, record: &log::Record) {
        let message = format!(
            "{}({}): {}",
            record.level(),
            record.module_path().unwrap_or_default(),
            record.args()
        );
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&message));
    }
}

/// Mount a element at the test location (creating/resetting it if needed)
/// # Panics
/// If the js is in a invalid state or the element is not found
pub fn mount_test<C: State>(state: C, tree: impl Element<C>) {
    let was_logger_active = LOGGER_ACTIVE.fetch_or(true, std::sync::atomic::Ordering::Relaxed);
    if !was_logger_active {
        log::set_logger(&SimpleLogger).expect("Failed to set logger");
        log::set_max_level(log::LevelFilter::Trace);
    }

    setup();

    log::debug!("Mounting test state {}", std::any::type_name::<C>());
    let result = render_state(state, tree, MOUNT_POINT).expect("Failed to mount");
    CURRENT_TREE.with(|cell| cell.set(Box::new(result)));
}

/// Setup `MOUNT_POINT` as a valid mount location
///
/// # Panics
/// if the js is in a invalid state.
pub fn setup() {
    core::clear();

    let document = web_sys::window()
        .expect("Failed to get window")
        .document()
        .expect("Failed to get document");

    if let Some(element) = document.get_element_by_id(MOUNT_PARENT) {
        log::trace!("Removed old test tree");
        element.remove();
    }

    let parent = document
        .create_element("div")
        .expect("Failed to create div");
    parent.set_id(MOUNT_PARENT);

    let mount = document
        .create_element("div")
        .expect("Failed to create div");
    mount.set_id(MOUNT_POINT);

    parent.append_child(&mount).expect("Failed to append child");
    document
        .body()
        .expect("Could not find <body>")
        .append_child(&parent)
        .expect("Failed to append child");

    log::trace!("Setup test target");
}

/// Get a html element based on id
///
/// # Panics
/// If js is in a invalid state or the element isnt found
#[must_use]
#[expect(clippy::panic, reason = "tests only")]
pub fn get(id: impl Into<&'static str>) -> HtmlElement {
    let id = id.into();

    let document = get_document();

    document
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("Id {id} not found"))
        .dyn_ref::<HtmlElement>()
        .expect("Target Node wasnt a html element")
        .clone()
}
