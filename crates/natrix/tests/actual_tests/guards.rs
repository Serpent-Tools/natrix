use natrix::prelude::*;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

const BUTTON: Id = natrix::id!();
const TEXT: Id = natrix::id!();

#[derive(State)]
struct TestOption {
    value: Signal<Option<u8>>,
}

fn render_test_option() -> impl Element<TestOption> {
    e::div()
        .child(
            e::button()
                .id(BUTTON)
                .on::<events::Click>(|mut ctx: EventCtx<TestOption>, _| match &mut *ctx.value {
                    Some(2) => *ctx.value = None,
                    Some(value) => *value += 1,
                    None => *ctx.value = Some(0),
                }),
        )
        .child(|mut ctx: RenderCtx<TestOption>| {
            if let Some(value_guard) = ctx.guard(field!(TestOption, .value).deref()) {
                e::div().text(move |mut ctx: RenderCtx<TestOption>| *value_guard.get(&ctx))
            } else {
                e::div().text("NO VALUE")
            }
            .id(TEXT)
        })
}

#[wasm_bindgen_test]
fn guard_works() {
    crate::mount_test(
        TestOption {
            value: Signal::new(None),
        },
        render_test_option(),
    );

    let button = crate::get(BUTTON);

    let text = crate::get(TEXT);
    assert_eq!(text.text_content(), Some("NO VALUE".to_owned()));

    button.click();
    let text = crate::get(TEXT);
    assert_eq!(text.text_content(), Some("0".to_owned()));

    button.click();
    assert_eq!(text.text_content(), Some("1".to_owned()));

    button.click();
    assert_eq!(text.text_content(), Some("2".to_owned()));

    button.click();
    let text = crate::get(TEXT);
    assert_eq!(text.text_content(), Some("NO VALUE".to_owned()));
}

#[derive(State)]
struct TestResult {
    value: Signal<Result<u8, u8>>,
}

fn render_test_result() -> impl Element<TestResult> {
    e::div()
        .child(
            e::button()
                .id(BUTTON)
                .on::<events::Click>(|mut ctx: EventCtx<TestResult>, _| match &mut *ctx.value {
                    Ok(value) => *value += 1,
                    Err(_) => *ctx.value = Ok(0),
                }),
        )
        .child(|mut ctx: RenderCtx<TestResult>| {
            match ctx.guard(field!(TestResult, .value).deref()) {
                Ok(value_guard) => {
                    e::div().text(move |mut ctx: RenderCtx<TestResult>| *value_guard.get(&ctx))
                }
                Err(error_guard) => {
                    e::div().text(move |mut ctx: RenderCtx<TestResult>| *error_guard.get(&ctx))
                }
            }
            .id(TEXT)
        })
}

#[wasm_bindgen_test]
fn guard_result() {
    crate::mount_test(
        TestResult {
            value: Signal::new(Err(100)),
        },
        render_test_result(),
    );

    let button = crate::get(BUTTON);

    let text = crate::get(TEXT);
    assert_eq!(text.text_content(), Some("100".to_owned()));

    button.click();
    let text = crate::get(TEXT);
    assert_eq!(text.text_content(), Some("0".to_owned()));

    button.click();
    assert_eq!(text.text_content(), Some("1".to_owned()));

    button.click();
    assert_eq!(text.text_content(), Some("2".to_owned()));
}

#[derive(State)]
struct Nested {
    value: Signal<Option<Option<u8>>>,
}

fn render_nested() -> impl Element<Nested> {
    e::div()
        .child(
            e::button()
                .id(BUTTON)
                .on::<events::Click>(|mut ctx: EventCtx<Nested>, _| match &mut *ctx.value {
                    Some(Some(2)) => *ctx.value = None,
                    Some(Some(value)) => *value += 1,
                    Some(None) => *ctx.value = Some(Some(0)),
                    None => *ctx.value = Some(None),
                }),
        )
        .child(|mut ctx: RenderCtx<Nested>| {
            if let Some(value_guard) = ctx.guard(field!(Nested, .value).deref()) {
                e::div().text(move |mut ctx: RenderCtx<Nested>| {
                    if let Some(inner_guard) = ctx.guard(value_guard) {
                        e::div()
                            .id(TEXT)
                            .text(move |mut ctx: RenderCtx<Nested>| *inner_guard.get(&ctx))
                    } else {
                        e::div().text("NO VALUE INNER").id(TEXT)
                    }
                })
            } else {
                e::div().text("NO VALUE").id(TEXT)
            }
        })
}

#[wasm_bindgen_test]
fn guard_nested() {
    crate::mount_test(
        Nested {
            value: Signal::new(None),
        },
        render_nested(),
    );

    let button = crate::get(BUTTON);

    let text = crate::get(TEXT);
    assert_eq!(text.text_content(), Some("NO VALUE".to_owned()));

    button.click();
    let text = crate::get(TEXT);
    assert_eq!(text.text_content(), Some("NO VALUE INNER".to_owned()));

    button.click();
    let text = crate::get(TEXT);
    assert_eq!(text.text_content(), Some("0".to_owned()));

    button.click();
    assert_eq!(text.text_content(), Some("1".to_owned()));

    button.click();
    assert_eq!(text.text_content(), Some("2".to_owned()));

    button.click();
    let text = crate::get(TEXT);
    assert_eq!(text.text_content(), Some("NO VALUE".to_owned()));
}
