# Html

Html elements are the building blocks of web pages. While other rust frameworks aim for a JSX-like syntax, this library uses a more traditional approach.
The goal is to provide a simple and efficient way to create HTML elements without the need for complex syntax, we use the idomatic rust builder pattern.

Natrix uses a single [`HtmlElement`](natrix::dom::html_elements::HtmlElement) struct to represent all HTML elements. But exposes helper functions for each tag.
These are found along side the `HtmlElement` struct in the [`html_elements`](natrix::dom::html_elements) module.
Which will most commonly be used via the `e` alias in the [`prelude`](natrix::prelude) module.

```rust,no_run
# extern crate natrix;
# use natrix::prelude::*;
# let _: e::HtmlElement<(), _> =
e::div()
# ;
```

If you need to construct a element with a tag not found in the library you can use [`HtmlElement::new`](natrix::dom::html_elements::HtmlElement::new).

```rust,no_run
# extern crate natrix;
# use natrix::prelude::*;
# let _: e::HtmlElement<(), ()> =
e::HtmlElement::new("custom_tag")
# ;
```

## Children

Children are added using the [`.child`](natrix::dom::html_elements::HtmlElement::child) method. This method takes a single child element and adds it to the parent element.

```rust,no_run
# extern crate natrix;
# use natrix::prelude::*;
# let _: e::HtmlElement<(), _> =
e::div()
    .child(e::button())
    .child(e::h1().child("Hello World!"))
# ;
```

> [!TIP]
> the [`.text`](natrix::dom::html_elements::HtmlElement::text) method is a alias for [`.child`](natrix::dom::html_elements::HtmlElement::child)

Child elements can be any type that implements the [`Element`](natrix::dom::element::Element) trait, including other [`HtmlElement`](natrix::dom::html_elements::HtmlElement) instances, and stdlib types like [`String`], [`&str`](std::primitive::str), [`i32`], as well as containers such as [`Option`] and [`Result`].

Child elements can also be reactive as closures implement the [`Element`](natrix::dom::element::Element) trait.

```rust
# extern crate natrix;
# use natrix::prelude::*;
#
# #[derive(State)]
# struct MyComponent {
#     pub is_active: Signal<bool>,
# }
#
# fn render(ctx: RenderCtx<MyComponent>) -> impl Element<MyComponent> {
e::div()
    .child(e::button()
        .text("Click me!")
        .on::<events::Click>(|mut ctx: EventCtx<MyComponent>, _| {
            *ctx.is_active = !*ctx.is_active;
        })
    )
    .child(|ctx: RenderCtx<MyComponent>| {
        if *ctx.is_active {
            Some(e::p().text("Active!"))
        } else {
            None
        }
    })
# }
```

## `format_elements`
You can use the [`format_elements`](natrix::format_elements) macro to get `format!` like ergonomics for elements.
```rust
# extern crate natrix;
# use natrix::prelude::*;
#
# #[derive(State)]
# struct MyComponent {
#     pub counter: Signal<u8>,
#     pub target: Signal<u8>,
# }
#
# fn render(ctx: RenderCtx<MyComponent>) -> impl Element<MyComponent> {
e::h1().children(natrix::format_elements!(
    |ctx: RenderCtx<MyComponent>| "Counter is {}, just {} clicks left!", 
    *ctx.counter, *ctx.target - *ctx.counter
))
# }
```
Which expands to effectively:
```rust
# extern crate natrix;
# use natrix::prelude::*;
#
# #[derive(State)]
# struct MyComponent {
#     pub counter: Signal<u8>,
#     pub target: Signal<u8>,
# }
#
# fn render(ctx: RenderCtx<MyComponent>) -> impl Element<MyComponent> {
e::h1()
    .text("Counter is ")
    .child(|ctx: RenderCtx<MyComponent>| *ctx.counter)
    .text(", just ")
    .child(|ctx: RenderCtx<MyComponent>| *ctx.target - *ctx.counter)
    .text(" clicks left!")
# }
```

I.e this is much more performant than `format!` for multiple reasons:
* You avoid the format machinery overhead.
* You get fine-grained reactivty for specific parts of the text.

This macro supports anything that is a element, including html elements.
```rust
# extern crate natrix;
# use natrix::prelude::*;
#
# #[derive(State)]
# struct MyComponent {
#     pub counter: Signal<u8>,
#     pub target: Signal<u8>,
# }
#
# fn render(ctx: RenderCtx<MyComponent>) -> impl Element<MyComponent> {
e::h1().children(natrix::format_elements!(
    |ctx: RenderCtx<MyComponent>| "Counter is {}, just {} clicks left!", 
    e::h1().text(*ctx.counter), *ctx.target - *ctx.counter
))
# }
```

## Attributes

Attributes are set using the [`.attr`](natrix::dom::html_elements::HtmlElement::attr) method. This method takes a key and a value, and sets the attribute on the element.

```rust,no_run
# extern crate natrix;
# use natrix::prelude::*;
# let _: e::HtmlElement<(), _> =
e::div()
    .attr("data-foo", "bar")
    .attr("data-baz", "qux")
# ;
```

Most standard html attributes have type-safe helper functions, for example `id`, `class`, `href`, `src`, etc.
For non-global attributes natrix only exposes them on the supporting elements.

```rust,no_run
# extern crate natrix;
# use natrix::prelude::*;
use natrix::dom::attributes;

# let _: e::HtmlElement<(), _> =
e::a()
    .href("https://example.com")
    .target(attributes::Target::NewTab) // _blank
    .rel(vec![attributes::Rel::NoOpener, attributes::Rel::NoReferrer])
# ;
```

But the following wont compile:

```rust,compile_fail
# extern crate natrix;
# use natrix::prelude::*;
# let _: e::HtmlElement<(), _> =
e::div()
    .target("_blank") // error: no method named `target` found for struct `HtmlElement<_, _div>`
# ;
```

Attributes can be set by anything that implements the [`ToAttribute`](natrix::dom::ToAttribute) trait, this includes numerics, [`Option`], and [`bool`], and others.
Attributes can also be reactive as closures implement the [`ToAttribute`](natrix::dom::ToAttribute) trait.

```rust,no_run
# extern crate natrix;
# use natrix::prelude::*;
#
# #[derive(State)]
# struct MyComponent {
#     pub is_active: Signal<bool>,
# }
#
# fn render(ctx: RenderCtx<MyComponent>) -> impl Element<MyComponent> {
e::button()
    .disabled(|ctx: RenderCtx<MyComponent>| !*ctx.is_active)
    .text("Click me!")
    .on::<events::Click>(|mut ctx: EventCtx<MyComponent>, _| {
        *ctx.is_active = !*ctx.is_active;
    })
# }
```

Importantly for the attribute helpers [`AttributeKind`](natrix::dom::attributes::ToAttribute::AttributeKind) determines what kind of values are allowed for that helper. Important a attribute kind of for example `bool` also supports `Option<bool>`, a closure returning `bool`, etc. For example this wont compile:
```rust,compile_fail
# extern crate natrix;
# use natrix::prelude::*;
# let _: e::HtmlElement<(), _> =
e::a()
    .target("_blank") // error: expected `attributes::Target`, found `&'static str`
# ;
```

## Classes

The [`.class`](natrix::dom::html_elements::HtmlElement::class) method is _not_ a alias for [`.attr`](natrix::dom::html_elements::HtmlElement::attr), it will add the class to the element, and not replace it. This is because the `class` attribute is a special case in HTML, and is used to apply CSS styles to elements. The [`.class`](natrix::dom::html_elements::HtmlElement::class) method will add the class to the element, and not replace any existing ones.

```rust,no_run
# extern crate natrix;
# use natrix::prelude::*;
#
const FOO: Class = natrix::class!(); // unique class name
const BAR: Class = natrix::class!();

# let _: e::HtmlElement<(), _> =
e::div()
    .class(FOO)
    .class(BAR)
# ;
```

Classes can also be reactive as closures implement the [`ToClass`](natrix::dom::ToClass) trait.

```rust
# extern crate natrix;
# use natrix::prelude::*;
#
const ACTIVE: Class = natrix::class!();

# #[derive(State)]
# struct MyComponent {
#     pub is_active: Signal<bool>,
# }
#
# fn render(ctx: RenderCtx<MyComponent>) -> impl Element<MyComponent> {
e::div()
    .class(|ctx: RenderCtx<MyComponent>| {
        if *ctx.is_active {
            Some(ACTIVE)
        } else {
            None
        }
    })
    .child(e::button()
        .text("Click me!")
        .on::<events::Click>(|mut ctx: EventCtx<MyComponent>, _| {
            *ctx.is_active = !*ctx.is_active;
        })
    )
# }
```
