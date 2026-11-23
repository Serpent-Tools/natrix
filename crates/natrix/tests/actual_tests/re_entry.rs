use natrix::format_elements;
use natrix::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{HtmlElement, PointerEvent};

wasm_bindgen_test_configure!(run_in_browser);

const BUTTON_ID: Id = natrix::id!();

#[derive(State)]
struct Counter {
    value: Signal<u8>,
}

fn explicit_reentry() -> impl Element<Counter> {
    let button = e::button().id(BUTTON_ID);
    let element = button.get_element().clone();
    button
        .text(|ctx: RenderCtx<Counter>| *ctx.value)
        .on::<events::Click>(move |mut ctx: EventCtx<Counter>, event: PointerEvent| {
            *ctx.value += 1;
            if *ctx.value < 10 {
                let event = web_sys::PointerEvent::new("click").expect("Should be valid");
                element.dispatch_event(&event).expect("Should work");
            }
        })
}

#[wasm_bindgen_test]
fn test_explicit_reentry() {
    crate::mount_test(
        Counter {
            value: Signal::new(0),
        },
        explicit_reentry(),
    );

    let button = crate::get(BUTTON_ID);
    button.click();
    assert_eq!(button.text_content(), Some("10".to_owned()));
}

const INPUT_ID: Id = natrix::id!();
const TEXT_ID: Id = natrix::id!();

fn implicit_reentry() -> impl Element<Counter> {
    e::div()
        .child(|ctx: RenderCtx<Counter>| {
            if *ctx.value == 0 {
                Some(
                    e::input()
                        .id(INPUT_ID)
                        .attr("data-foo", |ctx: RenderCtx<Counter>| *ctx.value)
                        .on::<events::Blur>(|mut ctx: EventCtx<Counter>, _| {
                            *ctx.value += 1;
                        }),
                )
            } else {
                None
            }
        })
        .child(
            e::h1()
                .id(TEXT_ID)
                .text(|ctx: RenderCtx<Counter>| *ctx.value)
                .on::<events::Click>(|mut ctx: EventCtx<Counter>, _| {
                    *ctx.value += 1;
                }),
        )
}

#[wasm_bindgen_test]
fn test_implicit_reentry() {
    crate::mount_test(
        Counter {
            value: Signal::new(0),
        },
        implicit_reentry(),
    );

    let input = crate::get(INPUT_ID);
    let text = crate::get(TEXT_ID);

    input.focus();
    text.click();

    assert_eq!(text.text_content(), Some("2".to_owned()));
}

#[cfg(all(feature = "async", feature = "async_utils"))]
mod async_rentry {
    use super::*;

    const BUTTON1_ID: Id = natrix::id!();
    const BUTTON2_ID: Id = natrix::id!();

    fn async_rentry() -> impl Element<Counter> {
        let button1 = e::button()
            .id(BUTTON1_ID)
            .text(|ctx: RenderCtx<Counter>| *ctx.value)
            .on::<events::Click>(|mut ctx: EventCtx<Counter>, _| {
                *ctx.value += 1;
            });
        let button1_element = button1.get_element().clone();

        e::div()
            .child(e::button().id(BUTTON2_ID).on::<events::Click>(
                move |mut ctx: EventCtx<Counter>, _| {
                    let button2_element = button1_element.clone();
                    ctx.use_async(async move |ctx| {
                        ctx.update(|mut ctx| {
                            *ctx.value += 1;
                        })?;

                        button2_element.click();
                        natrix::async_utils::next_animation_frame().await;

                        ctx.update(|mut ctx| {
                            button2_element.click();
                            *ctx.value += 1;
                        })?;

                        None
                    });
                },
            ))
            .child(button1)
    }

    #[wasm_bindgen_test]
    async fn test_async_reentry() {
        crate::mount_test(
            Counter {
                value: Signal::new(0),
            },
            async_rentry(),
        );

        let button1 = crate::get(BUTTON1_ID);
        let button2 = crate::get(BUTTON2_ID);

        button2.click();
        natrix::async_utils::sleep_milliseconds(20).await;

        assert_eq!(button1.text_content(), Some("4".to_owned()));
    }
}
