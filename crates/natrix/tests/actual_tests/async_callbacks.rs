#![cfg(feature = "async_utils")]

use std::time::Duration;

use natrix::async_utils;
use natrix::prelude::*;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

const BUTTON_ID: Id = natrix::id!();

#[derive(State)]
struct AsyncComponent {
    data: Signal<u8>,
}

fn render_async_component() -> impl Element<AsyncComponent> {
    e::button()
        .id(BUTTON_ID)
        .text(|ctx: RenderCtx<AsyncComponent>| *ctx.data)
        .on::<events::Click>(|mut ctx: EventCtx<AsyncComponent>, _| {
            ctx.use_async(async |ctx| {
                async_utils::sleep_milliseconds(10).await;
                ctx.update(|mut ctx| {
                    *ctx.data += 10;
                })?;
                Some(())
            });
        })
}

#[wasm_bindgen_test]
async fn async_works() {
    crate::mount_test(
        AsyncComponent {
            data: Signal::new(0),
        },
        render_async_component(),
    );

    let button = crate::get(BUTTON_ID);

    button.click();
    async_utils::sleep_milliseconds(15).await;
    assert_eq!(button.text_content(), Some("10".to_owned()));
}

#[wasm_bindgen_test]
async fn async_multiple() {
    crate::mount_test(
        AsyncComponent {
            data: Signal::new(0),
        },
        render_async_component(),
    );

    let button = crate::get(BUTTON_ID);

    button.click();
    button.click();
    button.click();
    async_utils::sleep_milliseconds(15).await;
    assert_eq!(button.text_content(), Some("30".to_owned()));
}

const BUTTON2: Id = natrix::id!();

#[derive(State)]
struct OptionalAsync {
    value: Signal<Option<u8>>,
}

fn render_optional_async() -> impl Element<OptionalAsync> {
    e::div()
        .child(
            e::button()
                .id(BUTTON_ID)
                .text(|ctx: RenderCtx<OptionalAsync>| format!("{:?}", *ctx.value))
                .on::<events::Click>(
                    |mut ctx: EventCtx<OptionalAsync>, _| match &mut *ctx.value {
                        None => *ctx.value = Some(0),
                        Some(x) => {
                            *x += 1;
                        }
                    },
                ),
        )
        .child(|mut ctx: RenderCtx<OptionalAsync>| {
            if let Some(guard) = ctx.guard(field!(OptionalAsync, .value).deref()) {
                Some(e::button().id(BUTTON2).on::<events::Click>(
                    move |mut ctx: EventCtx<OptionalAsync>, _| {
                        *ctx.value = None;
                        ctx.use_async(async move |ctx| {
                            natrix::async_utils::sleep_milliseconds(10).await;
                            let value: u8 = ctx.update(|mut ctx| {
                                let value = *guard.try_get(&mut ctx)?;
                                Some(value)
                            })??;
                            Some(())
                        });
                    },
                ))
            } else {
                None
            }
        })
}

#[wasm_bindgen_test]
async fn optional_async() {
    crate::mount_test(
        OptionalAsync {
            value: Signal::new(None),
        },
        render_optional_async(),
    );

    let button1 = crate::get(BUTTON_ID);
    assert_eq!(button1.text_content(), Some(String::from("None")));

    button1.click();
    assert_eq!(button1.text_content(), Some(String::from("Some(0)")));

    let button2 = crate::get(BUTTON2);
    button2.click();
    natrix::async_utils::sleep_milliseconds(20).await;
    assert_eq!(button1.text_content(), Some(String::from("None")));

    button1.click();
    assert_eq!(button1.text_content(), Some(String::from("Some(0)")));
}
