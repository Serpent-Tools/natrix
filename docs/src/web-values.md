# Web Values

Children, attributes, and classes are all built on the [`WebValue`](natrix::web_value::WebValue) trait, which is what lets them accept strings, numbers, `Option`, `Result`, closures, etc.
This chapter covers the trait itself, see the [Html chapter](html.md) for what each builder method accepts.

## Targets
`WebValue<T>` converts a value into the target `T` using [`WebValue::resolve`](natrix::web_value::WebValue::resolve).
Natrix has three targets:

* [`Node<C>`](natrix::dom::Node) for elements
* [`Attribute<C>`](natrix::dom::Attribute) for attribute values
* [`ClassName<C>`](natrix::dom::ClassName) for classes

[`Element<C>`](natrix::dom::element::Element) is a alias for `WebValue<Node<C>>`.

Targets implement [`RawWebValue`](natrix::web_value::RawWebValue), which defines the arguments passed to `resolve`, for example attributes get the attribute name and the element.

## Generic implementations
The implementations for std types are generic over the target, and require the target to implement certain traits:

* The target itself is a value of that target.
* `Result<O, E>` resolves whichever variant it holds.
* `Option<V>` resolves `V`, or `T::default()` for `None`, this requires `T: Default`.
* Strings and numerics are converted to a [`Cow<'static, str>`](std::borrow::Cow), this requires `T: From<Cow<'static, str>>`.

This means that for example strings are not valid classes because [`ClassName`](natrix::dom::ClassName) does not implement `From<Cow<'static, str>>`.

Closures taking a [`RenderCtx`](natrix::prelude::RenderCtx) are implemented separately for each target, as making them reactive requires target specific logic.

## Kinds
Every `WebValue` has a [`Kind`](natrix::web_value::WebValue::Kind), for example all strings have kind `String` and all integers have kind [`Integer`](natrix::web_value::Integer).
`Option` and closures use the kind of the value they wrap, and `Result<O, E>` has kind `(O::Kind, E::Kind)`.

Methods restrict the kinds they accept using [`SupportedBy`](natrix::web_value::SupportedBy), which supports accepting multiple kinds and handles the `Result` tuple.
For example a [`Float`](natrix::web_value::Float) bound also accepts integers:

```rust,no_run
# extern crate natrix;
# use natrix::prelude::*;
use natrix::dom::Attribute;
use natrix::web_value::{Float, SupportedBy, WebValue};

fn progress<C: State>(
    value: impl WebValue<Attribute<C>, Kind: SupportedBy<Float>>,
) -> impl Element<C> {
    e::progress().max(1.0).values(value)
}

# fn render() -> impl Element<()> {
e::div()
    .child(progress(0.5))
    .child(progress(1))
    .child(progress(Some(1_u8)))
    .child(progress(Ok::<f32, u8>(0.5)))
# }
```

```rust,compile_fail
# extern crate natrix;
# use natrix::prelude::*;
# use natrix::dom::Attribute;
# use natrix::web_value::{Float, SupportedBy, WebValue};
# fn progress<C: State>(
#     value: impl WebValue<Attribute<C>, Kind: SupportedBy<Float>>,
# ) -> impl Element<C> {
#     e::progress().max(1.0).values(value)
# }
# fn render() -> impl Element<()> {
progress("50%")
# }
```

List attributes such as `rel` use `SupportedBy<Vec<T>>`, which accepts both `Vec<T>` and `T`.

## Using `WebValue` in your own code
Functions can take the same bounds as the builder methods:

```rust,no_run
# extern crate natrix;
# use natrix::prelude::*;
use natrix::dom::{Attribute, ClassName};
use natrix::web_value::WebValue;

fn icon_button<C: State>(
    label: impl WebValue<Attribute<C>>,
    class: impl WebValue<ClassName<C>>,
    content: impl Element<C>,
) -> impl Element<C> {
    e::button().aria_label(label).class(class).child(content)
}
```

You can implement `WebValue` for your own types by delegating to a existing value.
For elements a function returning `impl Element<C>` is usually simpler, but a `WebValue` implementation allows the type to be used directly, for example in a `Option`.

```rust,no_run
# extern crate natrix;
# use natrix::prelude::*;
use natrix::dom::Node;
use natrix::web_value::WebValue;

struct Badge(u32);

impl<C: State> WebValue<Node<C>> for Badge {
    type Kind = Node<C>;

    fn resolve(self, arguments: ()) -> Node<C> {
        e::span().text(self.0).resolve(arguments)
    }
}

# let _: e::HtmlElement<(), _> =
e::div().child(Some(Badge(3)))
# ;
```
