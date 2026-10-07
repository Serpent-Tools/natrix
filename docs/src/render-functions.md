# Render Functions

Render functions are the core of how you build UI in Natrix. They are functions that return `impl Element<T>` where `T` is your state type. This chapter explains how to write and compose render functions effectively.

## Basic Render Functions

Every Natrix application starts with a render function. Elements are generic over the state type they work with:

```rust,no_run
# extern crate natrix;
# use natrix::prelude::*;

#[derive(State)]
struct MyApp {
    count: Signal<i32>,
}

fn render_my_app() -> impl Element<MyApp> {
    e::div()
        .child(e::h1().text("My App"))
        .child(e::p().text(|ctx: RenderCtx<MyApp>| *ctx.count))
}

fn main() {
    natrix::mount(MyApp { count: Signal::new(0) }, render_my_app);
}
```

## Passing Arguments

Since render functions are just regular Rust functions, they can take arguments:

```rust
# extern crate natrix;
# use natrix::prelude::*;


#[derive(State)]
struct MyApp {
    user_name: Signal<String>,
}

fn greeting(name: &'static str) -> impl Element<MyApp> {
    e::h1().text("Hello ").text(name)
}

fn render_my_app() -> impl Element<MyApp> {
    e::div()
        .child(greeting("World"))
        .child(e::p().text(|ctx: RenderCtx<MyApp>| ctx.user_name.clone()))
}
```

## Reactive Arguments

For reactive content, pass `impl Element<T>` as arguments instead of raw values. This allows fine-grained reactivity:

```rust
# extern crate natrix;
# use natrix::prelude::*;


#[derive(State)]
struct MyApp {
    user_name: Signal<String>,
}

fn greeting(name: impl Element<MyApp>) -> impl Element<MyApp> {
    e::h1().text("Hello ").child(name)
}

fn render_my_app() -> impl Element<MyApp> {
    e::div()
        .child(greeting(|ctx: RenderCtx<MyApp>| ctx.user_name.clone()))
}
```

Now only the name part will update when `user_name` changes, not the entire greeting.

## Event Handlers

You can pass event handlers to render functions using the [`EventHandler`](natrix::dom::events::EventHandler) trait:

```rust
# extern crate natrix;
# use natrix::prelude::*;
use natrix::dom::EventHandler;


#[derive(State)]
struct App {
    counter: Signal<i32>,
}

fn fancy_button(
    text: &'static str,
    on_click: impl EventHandler<App, events::Click>,
) -> impl Element<App> {
    e::button()
        .text(text)
        .on(on_click)
}

fn render_counter() -> impl Element<App> {
    e::div()
        .child(e::p().text(|ctx: RenderCtx<App>| *ctx.counter))
        .child(fancy_button("Increment", |mut ctx: EventCtx<App>, _| {
            *ctx.counter += 1;
        }))
}
```

## Direct state access
Since render functions are specialized on the state type, you can access the fields directly.

```rust
# extern crate natrix;
# use natrix::prelude::*;

#[derive(State)]
struct App {
    username: Signal<String>,
}

fn greeting() -> impl Element<App> {
    e::h1()
        .text("Hello ")
        .text(|ctx: RenderCtx<App>| ctx.username.clone())
}

fn render_app() -> impl Element<App> {
    e::div()
        .child(greeting())
        .child(e::p().text("Cool right?"))
}
```

## Generic state access.
What if you want a more reusable component, lets say a slider?
For this rust just uses closures, kinda.
Pure rust closures dont allow for combined read and mut paths, so natrix uses [`Ref`](natrix::access::Ref), see the [Getters Chapter](ref.md) for detailed docs on them.
[`Getter`](natrix::access::Getter) is a ergonomic "Alias Trait" for closures that take a `Ref` as the first argument, are cloneable, and are `'static`, all of which basically all closures in natrix need or might need at some point (and basically all closures will be), it saves you from having to write longer bounds, as well as needing to propagate a `Clone` bound after refactors as `Clone` is the default with `Getter`.  

```rust
# extern crate natrix;
# use natrix::prelude::*;

#[derive(State)]
struct App {
    first: Signal<u8>,
    second: Signal<u8>,
}

fn counter(value: impl Getter<App, u8>) -> impl Element<App> {
    e::div()
        .child(e::button().text("-").on::<events::Click>(with!(move value |mut ctx: EventCtx<App>, _| {
            *value.call_mut(&mut ctx) -= 1;
        })))
        .text(with!(move value |mut ctx: RenderCtx<App>| *value.call_read(&ctx)))
        .child(e::button().text("+").on::<events::Click>(with!(move value |mut ctx: EventCtx<App>, _| {
            *value.call_mut(&mut ctx) += 1;
        })))
}

fn render_app() -> impl Element<App> {
    e::div()
        .child(counter(|ctx| field!(ctx.first).deref()))
        .child(counter(|ctx| field!(ctx.second).deref()))
}
```
The amazing thing is that since the helper is still concrete (`impl Element<App>`), you can still access any field directly, if you said had a `theme` field, the `counter` function could access that directly without needing getters.

## Generic State Store
If you are writing a component library you naturally wont know the state, then you can simply use a generic:
```rust
# extern crate natrix;
# use natrix::prelude::*;

fn counter<S: State>(value: impl Getter<S, u8>) -> impl Element<S> {
    e::div()
        .child(e::button().text("-").on::<events::Click>(with!(move value |mut ctx: EventCtx<S>, _| {
            *value.call_mut(&mut ctx) -= 1;
        })))
        .text(with!(move value |mut ctx: RenderCtx<S>| *value.call_read(&ctx)))
        .child(e::button().text("+").on::<events::Click>(with!(move value |mut ctx: EventCtx<S>, _| {
            *value.call_mut(&mut ctx) += 1;
        })))
}
```

## Trait bound on state.
What if you want the nice feature of always having access to global state, but in a generic component library? well you can use traits!
```rust
# extern crate natrix;
use natrix::prelude::*;

mod my_library {
    use natrix::prelude::*;
    pub trait HasLanguage {
        fn get_language(&self) -> &'static str;
    }

    pub fn greeting<S>(name: impl Getter<S, str>) -> impl Element<S>
        where S: State + HasLanguage
    {
        e::div()
            .text(|ctx: RenderCtx<S>| {
                match ctx.get_language() {
                    "NO" => "Hallo ",
                    "EN" | _ => "Greetings "
                }
            })
            .text(move |mut ctx: RenderCtx<S>| name.call_read(&ctx).to_string())
    }
}

#[derive(State)]
struct App {
    lang: Signal<&'static str>,
    author_name: Signal<String>,
}

impl my_library::HasLanguage for App {
    fn get_language(&self) -> &'static str {
        &self.lang
    }
}

fn render_app() -> impl Element<App> {
    e::div()
        .child(my_library::greeting(|ctx: Ref<App>| field!(ctx.author_name).deref().deref()))
}
```
