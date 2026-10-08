# State

State in natrix usually refers to stuff implementing the [`State`](natrix::reactivity::State) trait.
This trait is intended to be implemented by the `State` derive macro, which will insert bounds to assert that all your fields are also `State`, and implement [`.set`](natrix::prelude::State::set) by setting each field.

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
You can easily nest state:
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
the [`Signal`](natrix::prelude::Signal) is the core reactive primitive in natrix, and implements read and write tracking on dereferencing.

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

### `Project`-ing signals
Signals support using [`Project`](natrix::access::Project) to access an inner value of a `Option`/`Result` without triggering re-renders for the entire inner value.
They do this using the [`.project`](natrix::access::Ref::project) method on `Ref`, as well as the [`.project_mut`](natrix::prelude::Signal::project_mut) method on signals themselves.

```rust
# extern crate natrix;
use natrix::prelude::*;

#[derive(State)]
struct User {
    name: Signal<String>,
    email: Signal<String>
}

#[derive(State)]
struct App {
    user: Signal<Option<User>>
}

fn render() -> impl Element<App> {
    e::div().child(|mut ctx: RenderCtx<App>| {
        if let Some(guard) = ctx.guard_option(|ctx| field!(ctx.user).project()) {
            e::h1().text(move |ctx: RenderCtx<App>| guard.call_read(&ctx).name.clone()).render()
        } else {
            "...".render()
        }
    })
    .on::<events::Click>(|mut ctx: EventCtx<App>, _| {
        if let Some(user) = ctx.user.project_mut() {
            *user.name = String::from("viv");
        }
    })
}
```

You can modify the `Option` itself using normal signal dereferencing, and the inner value using the `project_mut` method.
`as_mut` is also available on signals directly as an alias for `project_mut`.

> [!IMPORTANT]
> `project_mut` gives you a `&mut` to a `State`, so the [`.set`](natrix::prelude::State::set) rule applies:
> `*user = User { ... }` will not trigger reactive updates.

> [!WARNING]
> Due to the `DerefMut` implementation on `Signal` it is possible to mutate the inner value in a way that triggers re-renders of the entire inner value's readers,
> for example using [`Option::iter_mut`](std::option::Option::iter_mut) or [`Option::as_deref_mut`](std::option::Option::as_deref_mut).
>
> `DerefMut` is required for normal signal mutation, as well as allowing intentional usage of methods like [`.get_or_insert`](std::option::Option::get_or_insert).
