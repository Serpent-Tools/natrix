# State

State in natrix usually refers to stuff implementing the [`State`](natrix::reactivity::State) trait.
This trait is a **marker** trait, and is intended to be implemented by the `State` derive macro, which will insert bounds to assert that all your fields are also `State`.

```rust
# extern crate natrix;
# use natrix::prelude::*;

#[derive(State)]
struct App {
    counter: Signal<u8>,
}
```

For example the code below wont compile because `u8` is not `State`
```rust,compile_fail
# extern crate natrix;
# use natrix::prelude::*;

#[derive(State)]
struct App {
    counter: u8,
}
```

## Nesting state
You can easialy nest state:
```rust
# extern crate natrix;
# use natrix::prelude::*;

#[derive(State)]
struct Book {
    title: Signal<String>,
    author: Signal<String>,
}

#[derive(State)]
struct App {
    book: Book,
    user: Signal<String>,
}
```
Now you get fine-grained reactivity on the `book` fields.

> [!IMPORTANT]
> Never directly overwrite a `State`. Doing this will not trigger reactive updates. and will break your app.
> Use [`.set`](natrix::prelude::State::set) instead.
> ```rust
> # extern crate natrix;
> # use natrix::prelude::*;
> #
> # #[derive(State)]
> # struct App {
> #  counter: Signal<u8>
> # }
> # fn render() -> impl Element<App> {
> e::button().on::<events::Click>(|mut ctx: EventCtx<App>, _|{
>   // This is really bad:
>   ctx.counter = Signal::new(10);
> })
> # }
> ```
> This also includes overwriting any `State` struct directly, like `ctx.book = Book::...`

## `Signal`
the [`Signal`](natrix::prelude::Signal) is the core reactive primitive in natrix, and implements read and write tracking on derefrencing.

```rust
# extern crate natrix;
# use natrix::prelude::*;

#[derive(State)]
struct App {
    counter: Signal<u8>,
}

fn render() -> impl Element<App> {
    e::button()
        .text(|ctx: RenderCtx<App>| *ctx.counter) // The `*` read the `u8` value and tells natrix to track this
        .on::<events::Click>(|mut ctx: EventCtx<App>, _| {
            // Similarly this informs natrix the signal changed.
            *ctx.counter += 1;
        })
}
```

## `ProjectableSignal`
The [`ProjectableSignal`](natrix::reactivity::signal::ProjectableSignal) allows you to use fine-grained reactivity over certain wrapper types that dont implement the required tracking internally, such as most enums. When you have a `Ref` to the value you can use [`.project_signal`](natrix::access::Ref::project_signal) to get a projected `Ref` to the inner value.

```rust
# extern crate natrix;
use natrix::prelude::*;
use natrix::reactivity::signal::ProjectableSignal;

#[derive(State)]
struct User {
    name: Signal<String>,
    email: Signal<String>
}

#[derive(State)]
struct App {
    user: ProjectableSignal<Option<User>>
}

fn render() -> impl Element<App> {
    e::div().child(|mut ctx: RenderCtx<App>| {
        if let Some(guard) = ctx.guard_option(|ctx| field!(ctx.user).project_signal()) {
            e::h1().text(move |ctx: RenderCtx<App>| guard.call_read(&ctx).name.clone()).render()
        } else {
            "...".render()
        }
    })
}
```

Modify the option itself using `.update`/`.set`, but use `.as_mut`/`.project_signal` to modify the inner value.

> [!NOTE]
> Prefer `Signal<Option<NonState>>` over `ProjectableSignal<Option<Signal<NonState>>>`
