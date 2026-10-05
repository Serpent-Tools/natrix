use natrix::dom::Attribute;
use natrix::prelude::*;
use natrix::web_value::WebValue;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
wasm_bindgen_test_configure!(run_in_browser);

const ROOT: Id = natrix::id!();
const BUTTON: Id = natrix::id!();

#[derive(State, Default)]
struct Empty;

fn render_with_attr<T: WebValue<Attribute<Empty>>>(attr_value: T) -> impl Element<Empty> {
    e::div().attr("abc", attr_value).id(ROOT)
}

#[wasm_bindgen_test]
fn simple_true() {
    crate::mount_test(Empty, render_with_attr(true));

    let button = crate::get(ROOT);
    assert_eq!(button.get_attribute("abc"), Some("".to_owned()));
}
#[wasm_bindgen_test]
fn simple_false() {
    crate::mount_test(Empty, render_with_attr(false));

    let button = crate::get(ROOT);
    assert_eq!(button.get_attribute("abc"), None);
}
#[wasm_bindgen_test]
fn simple_string() {
    crate::mount_test(Empty, render_with_attr("hello"));

    let button = crate::get(ROOT);
    assert_eq!(button.get_attribute("abc"), Some("hello".to_owned()));
}
#[wasm_bindgen_test]
fn simple_some() {
    crate::mount_test(Empty, render_with_attr(Some("hello")));

    let button = crate::get(ROOT);
    assert_eq!(button.get_attribute("abc"), Some("hello".to_owned()));
}
#[wasm_bindgen_test]
fn simple_none() {
    crate::mount_test(Empty, render_with_attr(None::<u8>));

    let button = crate::get(ROOT);
    assert_eq!(button.get_attribute("abc"), None);
}

#[wasm_bindgen_test]
fn simple_ok() {
    crate::mount_test(
        Empty,
        render_with_attr(Ok::<&'static str, &'static str>("hello")),
    );

    let button = crate::get(ROOT);
    assert_eq!(button.get_attribute("abc"), Some("hello".to_owned()));
}
#[wasm_bindgen_test]
fn simple_err() {
    crate::mount_test(
        Empty,
        render_with_attr(Err::<&'static str, &'static str>("world")),
    );

    let button = crate::get(ROOT);
    assert_eq!(button.get_attribute("abc"), Some("world".to_owned()));
}

#[derive(State, Default)]
struct Counter {
    value: Signal<u8>,
}

impl Counter {
    fn increment(&mut self) {
        *self.value += 1;
    }
}

fn render_counter() -> impl Element<Counter> {
    e::button()
        .id(ROOT)
        .attr("abc", |ctx: RenderCtx<Counter>| format!("{}", *ctx.value))
        .on::<events::Click>(|mut ctx: EventCtx<Counter>, _| {
            ctx.increment();
        })
}

#[wasm_bindgen_test]
fn reactive_attribute() {
    crate::mount_test(Counter::default(), render_counter());

    let button = crate::get(ROOT);

    assert_eq!(button.get_attribute("abc"), Some("0".to_owned()));

    button.click();
    assert_eq!(button.get_attribute("abc"), Some("1".to_owned()));

    button.click();
    assert_eq!(button.get_attribute("abc"), Some("2".to_owned()));

    button.click();
    assert_eq!(button.get_attribute("abc"), Some("3".to_owned()));

    button.click();
    assert_eq!(button.get_attribute("abc"), Some("4".to_owned()));
}

#[derive(State, Default)]
struct Toggle {
    value: Signal<bool>,
}

impl Toggle {
    fn toggle(&mut self) {
        *self.value = !*self.value;
    }

    fn set_false(&mut self) {
        *self.value = false;
    }
}

fn render_toggle() -> impl Element<Toggle> {
    e::div()
        .child(
            e::button()
                .id(ROOT)
                .attr("abc", |ctx: RenderCtx<Toggle>| *ctx.value)
                .on::<events::Click>(|mut ctx: EventCtx<Toggle>, _| {
                    ctx.toggle();
                }),
        )
        .child(
            e::button()
                .id(BUTTON)
                .on::<events::Click>(|mut ctx: EventCtx<Toggle>, _| {
                    ctx.set_false();
                }),
        )
}

#[wasm_bindgen_test]
fn reactive_bool() {
    crate::mount_test(Toggle::default(), render_toggle());

    let button = crate::get(ROOT);

    assert_eq!(button.get_attribute("abc"), None);

    button.click();
    assert_eq!(button.get_attribute("abc"), Some("".to_owned()));

    button.click();
    assert_eq!(button.get_attribute("abc"), None);
}

#[wasm_bindgen_test]
fn reactive_change_set_but_no_change() {
    crate::mount_test(Toggle::default(), render_toggle());

    let button = crate::get(ROOT);
    let button2 = crate::get(BUTTON);

    assert_eq!(button.get_attribute("abc"), None);

    button.click();
    assert_eq!(button.get_attribute("abc"), Some("".to_owned()));

    button2.click();
    assert_eq!(button.get_attribute("abc"), None);

    button2.click();
    assert_eq!(button.get_attribute("abc"), None);
}
