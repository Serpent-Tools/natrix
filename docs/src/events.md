# Events

Events are one of the most important aspects of a web application. Natrix makes it super easy to update your state via event handlers using the [`on`](natrix::dom::html_elements::HtmlElement::on) method.
This method is generic over event marker types, which exist in the [`events`](natrix::prelude::events) module.

Event handlers, similar to render closures, take an [`EventCtx`](natrix::EventCtx), which is effectively a `&mut App`.
```rust
# extern crate natrix;
# use natrix::prelude::*;
#
# #[derive(State)]
# struct App {
#     pub is_active: Signal<bool>,
# }
#
# fn render(ctx: RenderCtx<App>) -> impl Element<App> {
e::button()
    .text("Click me!")
    .on::<events::Click>(|mut ctx: EventCtx<App>, _| {
        *ctx.is_active = !*ctx.is_active;
    })
# }
```

## Event argument

Event handlers also take the js event object, the specific one is indicated by the [`Event`](natrix::dom::events::Event) trait on the various marker types.
Because of some inference limitations, similar to the reason you sadly have to specify the context type, you have to specify the event type if you use it in your closure. You can either depend on `web_sys` (and enable the required feature), or use the `Event` trait as `<events::Click as Event>::JsEvent`.

```rust
# extern crate natrix;
# extern crate web_sys;
# use natrix::prelude::*;
#
# #[derive(State)]
# struct App {
#     pub is_active: Signal<bool>,
# }
#
# fn render(ctx: RenderCtx<App>) -> impl Element<App> {
e::button()
    .text("Click me!")
    .on::<events::Click>(|mut ctx: EventCtx<App>, event: web_sys::PointerEvent| {
        event.prevent_default();
        *ctx.is_active = !*ctx.is_active;
    })
# }
```

## Using an element in an event handler
As detailed in [the html chapter](html.md), html builders let you retrieve the js dom element using [`.get_element`](natrix::dom::html_elements::HtmlElement::get_element),
this can be cloned into event handlers to do stuff like moving focus.
```rust
# extern crate natrix;
# extern crate web_sys;
# use natrix::prelude::*;
#
# #[derive(State)]
# struct App {}
#
# fn render(ctx: RenderCtx<App>) -> impl Element<App> {
let first_button = e::button();
let first_button_element = first_button.get_element().clone();
e::div()
    .child(first_button)
    .child(
        e::button()
            .text("Click me!")
            .on::<events::Click>(move |_ctx: EventCtx<App>, _| {
                let _ = first_button_element.focus();
            })
    )
# }
```
> [!WARNING]
> As detailed in the html docs, you should not use this element reference to modify the dom tree as this can confuse natrix's reactivity engine.

## Re-Entry
Sometimes either on purpose or indirectly your event handlers will cause browser events to trigger, such as calling `.click`/`.focus` on an element, or a state change causing an element to be removed and hence trigger a `blur` event etc.
Often it might not be clear that re-entry might happen, such as below:
```rust
# extern crate natrix;
# extern crate web_sys;
# use natrix::prelude::*;
#
# #[derive(State)]
# struct App {
#   value: Signal<i32>
# }
#
# fn render(ctx: RenderCtx<App>) -> impl Element<App> {
e::div().child(|ctx: RenderCtx<App>| {
    if *ctx.value == 0 {
        Some(e::input()
          .on::<events::Blur>(|mut ctx: EventCtx<App>, _| { *ctx.value += 1; })
          .on::<events::KeyDown>(|mut ctx: EventCtx<App>, _| { *ctx.value += 1; })
        )
    } else {
        None
    }
})
# }
```
In the above case, when the `KeyDown` is triggered it will cause the `<input>` element to be destroyed by natrix, causing its `blur` handler to run *during natrix's dom update phase*.
When this happens, due to natrix's borrowing model, natrix will not be able to handle the event in the event's own js frame.
Natrix will instead run the event handler for the event as soon as it is able to, in other words right before returning from the outermost event handler.
This leads to a few things:

* Explicitly dispatching events from rust will not be reflected in the dom or state when the dispatch returns.
* `prevent_default` and similar will not work as the event handler runs outside the actual event frame.
* Handlers will see (and modify) state after other handlers have run, i.e. the state might not match what it was when the js event was triggered.

Naturally natrix would recommend you avoid doing event dispatches for their natrix side effects, in other words prefer methods on your state for shared behaviour instead, but some "implicit" re-entry will always occur, especially around focus state.

For example instead of doing the following:
```rust
# extern crate natrix;
# extern crate web_sys;
# use natrix::prelude::*;
#
#[derive(State)]
struct App {
    value: Signal<i32>
}

fn render(ctx: RenderCtx<App>) -> impl Element<App> {
    let button = e::button()
        .on::<events::Click>(|mut ctx: EventCtx<App>, _| {
            *ctx.value *= 2; // Some complex operation.
        });
    let button_element = button.get_element().clone();
    e::div()
        .child(button)
        .child(e::button().on::<events::Click>(move |mut ctx: EventCtx<App>, _| {
            *ctx.value += 1;
            button_element.click(); // I want to perform the operation of this other button.
        }))
}
```
It is much cleaner (and easier to test) to do:
```rust
# extern crate natrix;
# extern crate web_sys;
# use natrix::prelude::*;
#
#[derive(State)]
struct App {
    value: Signal<i32>
}

impl App {
    fn do_complex(&mut self) {
        *self.value *= 2; // some complex operation.
    }
}

fn render(ctx: RenderCtx<App>) -> impl Element<App> {
    e::div()
        .child(e::button().on::<events::Click>(|mut ctx: EventCtx<App>, _| {
            ctx.do_complex()
        }))
        .child(e::button().on::<events::Click>(|mut ctx: EventCtx<App>, _| {
            *ctx.value += 1;
            ctx.do_complex();
        }))
}
```
