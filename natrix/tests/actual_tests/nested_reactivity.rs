#![allow(dead_code)]

use natrix::prelude::*;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

const BUTTON_1: &str = "_BUTTON_1";
const BUTTON_2: &str = "_BUTTON_2";
const TEXT: &str = "_TEXT";

#[derive(Component, Default)]
struct DoulbeCounter {
    value_one: u8,
    value_two: u8,
}

impl Component for DoulbeCounter {
    fn render() -> impl Element<Self::Data> {
        e::div()
            .child(
                e::button()
                    .id(BUTTON_1)
                    .on::<events::Click>(|ctx: &mut S<Self>, _| {
                        *ctx.value_one += 1;
                    }),
            )
            .child(
                e::button()
                    .id(BUTTON_2)
                    .on::<events::Click>(|ctx: &mut S<Self>, _| {
                        *ctx.value_two += 1;
                    }),
            )
            .child(|ctx: R<Self>| {
                (*ctx.value_one >= 2)
                    .then_some(e::div().id(TEXT).child(|ctx: R<Self>| *ctx.value_two))
            })
    }
}

#[wasm_bindgen_test]
fn update_affects_inner_node() {
    crate::setup();
    mount_component(DoulbeCounter::default(), crate::MOUNT_POINT);

    let button_1 = crate::get(BUTTON_1);
    let button_2 = crate::get(BUTTON_2);

    button_1.click();
    button_1.click();
    button_1.click();
    button_1.click();

    let text = crate::get(TEXT);
    assert_eq!(text.text_content(), Some("0".to_owned()));

    button_2.click();
    assert_eq!(text.text_content(), Some("1".to_owned()));

    button_2.click();
    assert_eq!(text.text_content(), Some("2".to_owned()));

    button_2.click();
    assert_eq!(text.text_content(), Some("3".to_owned()));

    button_2.click();
    assert_eq!(text.text_content(), Some("4".to_owned()));

    button_2.click();
    assert_eq!(text.text_content(), Some("5".to_owned()));
}
