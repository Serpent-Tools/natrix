use natrix::prelude::*;
use wasm_bindgen_test::*;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

const HELLO_ID: Id = natrix::id!();

#[derive(State)]
struct Empty;

#[wasm_bindgen_test]
fn renders_fine() {
    crate::mount_test(Empty, e::h1().id(HELLO_ID).text("Hello World!"));

    let element = crate::get(HELLO_ID);
    assert_eq!(element.text_content(), Some("Hello World!".to_owned()));
}

#[wasm_bindgen_test]
fn render_option_some() {
    crate::mount_test(Empty, e::div().id(HELLO_ID).child(Some("hey")));

    let element = crate::get(HELLO_ID);
    assert_eq!(element.text_content(), Some("hey".to_owned()));
}

#[wasm_bindgen_test]
fn render_option_none() {
    crate::mount_test(Empty, e::div().id(HELLO_ID).child(None::<String>));

    let element = crate::get(HELLO_ID);
    assert_eq!(element.text_content(), Some("".to_owned()));
}

#[wasm_bindgen_test]
fn render_result_ok() {
    crate::mount_test(Empty, e::div().id(HELLO_ID).child(Ok::<&str, &str>("hey")));

    let element = crate::get(HELLO_ID);
    assert_eq!(element.text_content(), Some("hey".to_owned()));
}

#[wasm_bindgen_test]
fn render_result_err() {
    // Different types on purpose to test that the type system allows that.
    crate::mount_test(Empty, e::div().id(HELLO_ID).child(Err::<i32, &str>("hey")));

    let element = crate::get(HELLO_ID);
    assert_eq!(element.text_content(), Some("hey".to_owned()));
}

#[cfg(feature = "either")]
mod either_test {
    use either::Either;

    use super::*;

    #[wasm_bindgen_test]
    fn render_either_left() {
        crate::mount_test(
            Empty,
            e::div()
                .id(HELLO_ID)
                .child(Either::Left::<&str, &str>("hey")),
        );

        let element = crate::get(HELLO_ID);
        assert_eq!(element.text_content(), Some("hey".to_owned()));
    }

    #[wasm_bindgen_test]
    fn render_either_right() {
        crate::mount_test(
            Empty,
            e::div()
                .id(HELLO_ID)
                .child(Either::Right::<&str, &str>("hey")),
        );

        let element = crate::get(HELLO_ID);
        assert_eq!(element.text_content(), Some("hey".to_owned()));
    }
}
