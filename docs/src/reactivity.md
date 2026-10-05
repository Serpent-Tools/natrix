# Reactivity

## Callbacks

You have already seen `|ctx: RenderCtx<App>| ...` used in the varying examples in the book.
Lets go into some more detail about what this does. Both `RenderCtx` and `EventCtx` implement `Deref` to your `App` (and `DerefMut` as well for `EventCtx`)

> ![TIP]
> You can define type aliases for the two context types specialized on your type
> ```rust
> # extern crate natrix;
> # use natrix::prelude::*;
> #[derive(State)]
> struct App { /* ... */ }
> type E<'s> = EventCtx<'s, App>;
> type R<'s, 'r> = RenderCtx<'s, 'r, App>;
> ```

The callbacks return a value that implements [`Element`](natrix::dom::element::Element), internally the framework will register which fields you accessed.
And when those fields change, the framework will recall the callback and update the element with the result.

Natrix does **_not_** use a virtual dom, meaning when a callback is re-called the framework will swap out the entire associated element.

> [!IMPORTANT]
> Natrix assumes render callbacks are pure, meaning they do not have side effects.
> If you use stuff like interior mutability it will break framework assumptions,
> and will likely lead to panics or desynced state.

### Execution Guarantees

Natrix _only_ makes the following guarantees about when a callback will be called:

- It will not be called if a parent is dirty.

Thats it, natrix does not make any guarantees about the order of sibling callbacks.

## Returning different kinds of elements.
Sometimes two branches returns different kinds of elements, this can be solved using `Result`, or by pre-rendering them using [`.render`](natrix::dom::element::Element::render). Which produces the internal result of a element render (which itself implements `Element` for this exact purpose)

```rust
# extern crate natrix;
# use natrix::prelude::*;
# #[derive(State)]
# struct HelloWorld {
#     counter: Signal<u8>,
# }
# fn render_hello_world() -> impl Element<HelloWorld> {
e::div()
    .child(|ctx: RenderCtx<HelloWorld>| {
        if *ctx.counter > 10 {
            e::h1().text("Such big").render()
        } else {
            "Oh no such small".render()
        }
    })
# }

```

```rust
# extern crate natrix;
# use natrix::prelude::*;
# #[derive(State)]
# struct HelloWorld {
#     counter: Signal<u8>,
# }
# fn render_hello_world() -> impl Element<HelloWorld> {
e::div()
    .child(|ctx: RenderCtx<HelloWorld>| {
        if *ctx.counter > 10 {
            Ok(e::h1().text("Such big"))
        } else {
            Err("Oh no such small")
        }
    })
# }

```

> [!TIP]
> For handling multiple types of html elements, theres [`.generic()`](natrix::dom::html_elements::HtmlElement::generic), which returns `HtmlElement<C, ()>`, i.e erases the dom tag, allowing you to for example construct different tags in a `if`, and then later call methods on it. Ofc doing this means only global attribute helpers can be used, but you can always use `.attr` directly. 

## Signal-based Reactivity

Natrix uses `Signal<T>` types to track reactive state. When you access a signal in a render callback, the framework automatically tracks the dependency and will re-run the callback when the signal changes.

```rust
# extern crate natrix;
# use natrix::prelude::*;
#[derive(State)]
struct Counter {
    value: Signal<i32>,
}

fn render_counter() -> impl Element<Counter> {
    e::div()
        .child(e::button()
            .text(|ctx: RenderCtx<Counter>| *ctx.value)
            .on::<events::Click>(|mut ctx: EventCtx<Counter>, _| {
                *ctx.value += 1;
            }))
}
```

The reactivity system automatically tracks when `ctx.value` is accessed and will re-run the text callback whenever the value changes.

## Computed values
What if you have something that depends on a computed value? if you did `if *ctx.value > 2` then that reactive closure would re-run whenever `.value` changes.
This is where [`ctx.watch`](natrix::prelude::RenderCtx::watch) comes in, this caches the result of the computation and only re-runs the parent closure if the calculated value changes.

```rust
# extern crate natrix;
# use natrix::prelude::*;
# #[derive(State)]
# struct App {value: Signal<u32>}
#
# fn render() -> impl Element<App> {
|mut ctx: RenderCtx<App>| {
    if ctx.watch(|ctx| *ctx.value > 2) {
        e::button().text(|ctx: RenderCtx<App>| *ctx.value).generic()
    } else {
        e::h1().text("Value is too low").generic()
    }
}
# }
```

Here the `*ctx.value > 2` will re-run whenever `ctx.value` changes, *but* the if-block itself will only-run if the condition flips, which in practice means we arent swapping out dom-nodes all the time.

## Guards - Handling `Option`/`Result`

Guards provide a way to safely access the inner value of `Option` or `Result` types while maintaining fine-grained reactivity. They solve a common problem when working with optional values in reactive contexts.

### The Problem with Optional Values

Consider this common pattern when working with optional values:

```rust
# extern crate natrix;
# use natrix::prelude::*;
#[derive(State)]
struct App {
    value: Signal<Option<u32>>,
}

fn render() -> impl Element<App> {
    |ctx: RenderCtx<App>| {
        if let Some(value) = *ctx.value {
            e::div().text(value)
        } else {
            e::div().text("Is none")
        }
    }
}
```

**The issue:** The outer div gets recreated every time `value` changes, even when it's just the inner value changing like `Some(0) -> Some(1)`. This happens because the entire expression is reactive to any change in `ctx.value`.

### First Attempt: Using `ctx.watch`

You might reach for `ctx.watch` to solve this, and it actually works perfectly:

```rust
# extern crate natrix;
# use natrix::prelude::*;
# #[derive(State)]
# struct App {value: Signal<Option<u32>>}
# fn render() -> impl Element<App> {
# |mut ctx: RenderCtx<App>| {
if ctx.watch(|ctx| ctx.value.is_some()) {
    e::div().text(|ctx: RenderCtx<App>| ctx.value.unwrap())
} else {
    e::div().text("Is none")
}
# }}
```

Now a change from `Some(0)` to `Some(1)` will only run the inner closure, and the outer div is reused.

**But there's a downside:** We need `.unwrap()` because the inner closure is technically isolated from the outer condition. This is:
- Ugly and error-prone
- Easy to forget the outer condition exists
- Creates a potential panic point

### Enter Guards

Guards provide an elegant solution to this exact problem:

```rust
# extern crate natrix;
# use natrix::prelude::*;
# #[derive(State)]
# struct App {value: Signal<Option<u32>>}
# fn render() -> impl Element<App> {
# |mut ctx: RenderCtx<App>| {
if let Some(value_guard) = ctx.guard_option(|ctx| field!(ctx.value).deref().project()) {
    e::div().text(move |mut ctx: RenderCtx<App>| *value_guard.call_read(&ctx))
} else {
    e::div().text("Is none")
}
# }}
```

Here `value_guard` is **not** the actual value—it's a [lens](lens.md) that can be captured by child closures.
Specifically `ctx.guard(...)` on a lens for `Option<T>` will return a `Option<impl Lens<..., T>>`.

### How Guards Work

Guards use [`ctx.watch`](natrix::prelude::RenderCtx::watch) internally to track when the condition changes (like `.is_some()`), but they provide a safe way to access the inner value without `.unwrap()`.

When you use `ctx.get(value_guard)`, you get the inner value safely because the guard guarantees it exists.

> [!NOTE]
> Internally, guards do use `.unwrap()`, but it should never fail because the guard's existence guarantees the value is `Some`.

### Guards with Results

Guards also work with `Result<T, E>` types:

```rust
# extern crate natrix;
# use natrix::prelude::*;
# #[derive(State)]
# struct App {operation: Signal<Result<u32, String>>}
# fn render() -> impl Element<App> {
# |mut ctx: RenderCtx<App>| {
match ctx.guard_result(|ctx| field!(ctx.operation).deref().project()) {
    Ok(success_guard) => {
        e::div()
            .text(move |mut ctx: RenderCtx<App>| *success_guard.call_read(&ctx))
    }
    Err(error_guard) => {
        e::div()
            .text(move |mut ctx: RenderCtx<App>| error_guard.call_read(&ctx).clone())
    }
}
# }}
```
