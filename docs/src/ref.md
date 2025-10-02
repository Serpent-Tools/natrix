# Getters

Natrix uses [`Ref`](access::Ref), which is effectively a enum over `&T` and `&mut T`.
This is to make getters easier to work with and write.

## Why?
As you know natrix uses a global state store arithecture, 
which means to make components generic over which field they use we use getter closures.
The issue is, we want the same getter to work for both `&` and `&mut`, this is where `Ref` comes in.
A getter closure should return the same variant that it was given, which it will do if you stick to the `Ref` methods.

## How to use

The core way you use `Ref` is using `impl` bounds in your components:
```rust
# extern crate natrix;
# use natrix::prelude::*;
# #[derive(State)]
# struct App;
fn counter(value: impl Fn(Ref<App>) -> Ref<u8> + 'static) -> impl Element<App> {
    /* ... */
#   e::div()
}
```

### `Getter`
the [`Getter`](access::Getter) trait is a alias for the most common form of getter.
`impl Getter<A, B>` is `impl Fn(Ref<A>) -> Ref<B> + Clone + 'static`, 

```rust
# extern crate natrix;
# use natrix::prelude::*;
# #[derive(State)]
# struct App;
fn counter(value: impl Getter<App, u8>) -> impl Element<App> {
    /* ... */
#   e::div()
}
```

> [!NOTE]
> `Getter` does not cover every valid getter, but it covers the most commonly needed form.
> For example if you need `Option<Ref<T>>` you need to do that with `Fn` directly, 
> `impl Fn(Ref<App>) -> Option<Ref<u8>> + Clone + 'static`

### `.call_read`/`.call_mut`
Calling these closures directly would require unwrapping on the result, even tho all valid closures should return a know variant.
For this we provide the [`RefClosure`](access::RefClosure) trait, which provides the `.call_read` and `.call_mut` methods
which wrap your reference in the appropriate variant and unwraps on the result. 

```rust
# extern crate natrix;
# use natrix::prelude::*;
# #[derive(State)]
# struct App;
fn counter(value: impl Getter<App, u8>) -> impl Element<App> {
    e::button()
        .text(with!(move value |mut ctx: RenderCtx<App>| *value.call_read(&ctx)))
        .on::<events::Click>(with!(move value |mut ctx: EventCtx<App>, _| {
            *value.call_mut(&mut ctx) += 1;
        }))
}
```
`RefClosure` is implemented for most closures that take a `Ref` and return a type *containing* a `Ref`.
More specifically, if the return type implements [`Downgrade`](access::Downgrade).
For example using `.call_read` with a closure returning `Option<Ref<T>>` will yield a `Option<&T>`

### `.map`
the [`.map`](access::Ref::map) method takes two closures, a read and a write one, and apply them to the `Ref` depending on the current variant.
```rust
# extern crate natrix;
# use natrix::prelude::*;
#[derive(State)]
struct App {
    value: Signal<u8> 
};

fn counter(value: impl Getter<App, u8>) -> impl Element<App> {
    /* ... */
#   e::div()
}

fn render() -> impl Element<App> {
    e::div()
        .child(counter(|ctx| ctx.map(
            // We deref to turn the `&Signal<u8>` into a `&u8`
            |ctx| &*ctx.value,
            |ctx| &mut *ctx.value
        )))
}
```
Generally you would only need to use this if `Ref` doesnt already provide a abstraction for the operation you wish to perform. 

### `field!`

As you saw above `.map` gets verbose when doing stuff like field access.
The `field!` macro simplifies this. `field!(ctx.user.name)` expands to `ctx.map(|ctx| &ctx.user.name, |ctx| &mut ctx.user.name)`.
```rust
# extern crate natrix;
# use natrix::prelude::*;
# #[derive(State)]
# struct App {
#    value: Signal<u8> 
# };
#
# fn counter(value: impl Getter<App, u8>) -> impl Element<App> {
#   e::div()
# }
# 
fn render() -> impl Element<App> {
    e::div()
        .child(counter(|ctx| field!(ctx.value).deref()))
}
```
The `field!` macro also support a expression as the value, but this requires using `()`.
`field!((user_getter(ctx)).favorite_book.title)`

### `Deref`
Naturally you will often hit `Signal` in your access chains, [`.deref`](access::Ref::deref) can be used if the value implements both `Deref` and `DerefMut`, which `Signal` does. Or for example if you have a `String` and a component wants `Ref<str>`.


### `.project`
Often it can be useful to transform from `Ref<Option<T>>` to `Option<Ref<T>>`, this is where [`.project`](access::Ref::project) and the [`Project`](access::Project) trait come in.
They allow you to do these transformations in your getters, for example if a function wants `Option<Ref<T>>` and you have a `Ref<Option<T>>` you can use `.project()`.
This is very often used with [`.guard_*`](prelude::RenderCtx::guard_option).

### `with!`
Often we dont want to add `Copy` bounds to the getters without reason, but cloning gets annoying. You usually find yourself needing to do:
```rust
# extern crate natrix;
# use natrix::prelude::*;
# fn foo(getter: impl Fn(u8) + Clone) {
let getter = getter.clone();
let new_getter = move |value| getter(value);
# }
```
the `with!` macro makes this easier
```rust,no_run
# extern crate natrix;
# use natrix::with;
# let getter = |value: u8| value * 2;
let foo = with!(move getter |value: u8| getter(value) + 10);
// Expands to:
let foo = {
    let getter = getter.clone();
    move |value: u8| getter(value) + 10
};
```

If you want to move multiple values into a closure you can use `()` and list them out:
`with!(move (foo, bar) |...| ...)`

## Best Practices

> [!NOTE]
> These suggestions are mainly intended for libraries to help them write the most flexible signatures they can.
> But they are not a bad idea to adopt in application code either.

### `Option<Ref<T>>` vs `Ref<Option<T>>`
If you only need to know if the value is `Some`/`None`, but not modify the variant,
you should opt for `Option<Ref<T>>`, as this allow for example a getter accessing a `Result<T, E>` to be used (via `.project().ok()`).
Similar prinicibles apply in general.

### `T` vs `Ref<T>`
Generally if the value is cheap to clone/copy and you only need read access a direct owned `T` is often better.
As this allows the value to be a constant, or even computed.

## Advanced

> [!WARNING]
> This section documents the various invariants and connventions for more low level usage of `Ref`
> Most user code does not need to care about this.

### Core invariants
* All closures and methods dealing with `Ref` must maintain its variant. A closure must never return a `Ref::FailableMut` when given a `Ref::Mut` for example.

### `Ref::FailableMut` 
This variant is used in async context, which allows stuff like guards to return `None` in those cases.
While still keeping their ergonomics (i.e internal panics) in sync code.
When writing closures you have no way of knowing weather code will contains guards in the past or future, 
and hence should always take extra care to make sure the propagation of `FailableMute` makes sense.

### Project
At its core `Project` is simply a transformation from `Ref<Self>` to another type, usually the same as `Self`, but with different generics.
Such as `Ref<Option<T>>` into `Option<Ref<T>>`.
The more important point to pay attention to is with `FailableMut` handling.
Take this implementation of `Project` for options:
```rust,ignore
impl<T> Project for Option<T> {
    type Projected<'a>
        = Option<Ref<'a, T>>
    where
        Self: 'a;

    fn project(value: Ref<'_, Self>) -> Self::Projected<'_> {
        match value {
            Ref::Read(value) => value.as_ref().map(Into::into),
            Ref::Mut(value) => value.as_mut().map(Into::into),
            Ref::FaillableMut(None) => Some(Ref::FaillableMut(None)),
            Ref::FaillableMut(Some(value)) => {
                value.as_mut().map(|value| Ref::FaillableMut(Some(value)))
            }
        }
    }
}
```
As you see for a failed mut we return `Some(FaillableMut(None))`, why?  
The core reason is that if we returned `None` then the `call_failable` call would actually return `Some(None)`, instead of the intended `None`,
Because theres not way for it to know the `faillableMut` failed. Hence the general rule is to return a variant containing a `FaillableMut(None)`.
This works for `Result` as well (which uses `Ok` for this case).
If the value is later used in a guard closure, weather thats `Err` or `Ok` variant it will work out in the end.
if it expects `Ok`, then it sucesffuly grabs the inner `FaillableMut(None)`, which when it gets to downgrade will result in `None`.
if it expects a `Err`, then its logic will return `FaillableMut(None)` explicitly, which again means the final result is a `None`.

### Downgrade
Downgrading is the act of converting all references in a struct/enum to the specified reference type.
`Downgrade` is implemented for `Ref` and `&mut`, and more importantly for `Option` and `Result`.
In the `into_read` method you should return `None` if you get `FaillableMut(None)`, otherwise return the downgraded reference.
In `into_mut`, return `None` for `FaillableMut(None)` and `Read`.

*`Downgrade` should only be implemented for types that could represent both immutable and mutable references, for example its explicitly not implemented for `&`*
