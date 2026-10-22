# Optics

Natrix uses [`Optic`](natrix::lens::Optic)s to write reusable components that can both read and modify a location in the state.

## Why?
As you know natrix uses a global state store architecture,
which means to make components generic over which field they use we need some form of getter.
The issue is, we want the same getter to work for both `&` and `&mut`, a plain closure can only do one of the two.
An optic is effectively a pair of closures, one for the read path and one for the mut path, that both point to *the same location*.

## How to use

The core way you use optics is using `impl` bounds in your components:
```rust
# extern crate natrix;
# use natrix::prelude::*;
# #[derive(State)]
# struct App;
fn counter(value: impl Optic<&'static App, Output = &'static u8>) -> impl Element<App> {
    /* ... */
#   e::div()
}
```
The generic parameter is the input [`Shape`](natrix::lens::Shape), and `Output` is the output shape.
`&'static T` is the shape of a plain reference, which is specialized to `&T` or `&mut T` depending on which path is used.

### `Lens`
The [`Lens`](natrix::lens::Lens) trait is implemented for every optic that outputs a reference, and is the most common bound you will write.
`impl Lens<&'static App, Target = u8>` is equivalent to the bound above.

```rust
# extern crate natrix;
# use natrix::prelude::*;
# #[derive(State)]
# struct App;
fn counter(value: impl Lens<&'static App, Target = u8>) -> impl Element<App> {
    /* ... */
#   e::div()
}
```

### `.get`/`.get_mut`
Optics are executed using [`.get`](natrix::lens::Optic::get) for the read path and [`.get_mut`](natrix::lens::Optic::get_mut) for the mut path.
Optics are `Copy`, so they can be moved into as many closures as you need.

```rust
# extern crate natrix;
# use natrix::prelude::*;
# #[derive(State)]
# struct App;
fn counter(value: impl Lens<&'static App, Target = u8>) -> impl Element<App> {
    e::button()
        .text(move |mut ctx: RenderCtx<App>| *value.get(&ctx))
        .on::<events::Click>(move |mut ctx: EventCtx<App>, _| {
            *value.get_mut(&mut ctx) += 1;
        })
}
```

### `field!`
The [`field!`](natrix::field) macro creates an optic accessing a field of the given type.
`field!(App, .user.name)` is an optic from `&App` to `&String`.
```rust
# extern crate natrix;
# use natrix::prelude::*;
#[derive(State)]
struct App {
    value: Signal<u8>,
}

fn counter(value: impl Lens<&'static App, Target = u8>) -> impl Element<App> {
    /* ... */
#   e::div()
}

fn render() -> impl Element<App> {
    e::div()
        .child(counter(field!(App, .value).deref()))
}
```

### `.then`
[`.then`](natrix::lens::Optic::then) chains two optics together, this is used to chain `field!` onto an existing optic,
as well as to re-use the same optic from multiple roots.
[`root`](natrix::lens::root) returns an optic that returns its input unchanged, which is useful to start a chain.
```rust
# extern crate natrix;
# use natrix::prelude::*;
struct Book {
    name: String,
}

struct App {
    book1: Book,
    book2: Book,
}

let book_to_name = root::<&Book>().then(field!(Book, .name)).deref();
let book1_name = field!(App, .book1).then(book_to_name);
let book2_name = field!(App, .book2).then(book_to_name);
```

### `.deref`
Naturally you will often hit `Signal` in your access chains, [`.deref`](natrix::lens::Lens::deref) can be used if the value implements both `Deref` and `DerefMut`, which `Signal` does.
Or for example if you have a `String` and a component wants `&str`.

### `.ref_map`
The [`.ref_map`](natrix::lens::Lens::ref_map) method takes two closures, a read and a write one.
Generally you would only need to use this if natrix doesn't already provide an optic for the operation you wish to perform, for example getters on a third party type.
```rust
# extern crate natrix;
# use natrix::prelude::*;
struct Book {
    name: String,
}

impl Book {
    fn name(&self) -> &String {
        &self.name
    }

    fn name_mut(&mut self) -> &mut String {
        &mut self.name
    }
}

let book_name = root::<&Book>().ref_map(Book::name, Book::name_mut);
```

> [!WARNING]
> The two closures must point to the same location, natrix can not check this for you.

### `.project`
Often it can be useful to transform from `&Option<T>` to `Option<&T>`, this is where [`.project`](natrix::lens::Lens::project) and the [`Projectable`](natrix::lens::Projectable) trait come in.
For example if a component wants an `impl Optic<&'static App, Output = Option<&'static u8>>` and you have a `Signal<Option<u8>>` you can use `field!(App, .value).deref().project()`.

`Signal` also implements `Projectable` when it contains a `State`, see [Projecting signals](state.md#project-ing-signals).

### `.try_get`
[`.try_get`](natrix::lens::Optic::try_get) runs the mut path, but returns `None` if any part of the chain fails, such as a [guard](reactivity.md#guards---handling-optionresult) whose value is no longer present.
This must be used in async code, see [the async chapter](async-components.md#optics).

## Best Practices

> [!NOTE]
> These suggestions are mainly intended for libraries to help them write the most flexible signatures they can.
> But they are not a bad idea to adopt in application code either.

### `Option<&T>` vs `&Option<T>`
If you only need to know if the value is `Some`/`None`, but not modify the variant,
you should opt for an output of `Option<&T>`, as this allows more sources to be used,
for example an optic accessing the `Ok` value of a `Signal<Result<T, E>>`.
Similar principles apply in general.

### `T` vs optics
Generally if the value is cheap to clone/copy and you only need read access a direct owned `T` is often better.
As this allows the value to be a constant, or even computed.
