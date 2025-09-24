> [!IMPORTANT]
> Natrix is a work in progress and not suitable for production yet.

# Introduction

<img src="https://raw.githubusercontent.com/Serpent-Tools/branding/refs/heads/main/natrix.svg" alt="Logo" width="300" height="300">

Natrix is a ***Rust-first*** frontend framework. Embracing Rust’s strengths—leveraging smart pointers, derive macros, the builder pattern, and other idiomatic Rust features to create a truly native experience.

# A Simple Example
A simple counter in Natrix looks like this: 
```rust
use natrix::prelude::*;

#[derive(State)]
struct Counter {
    value: Signal<usize>,
}

fn render_counter() -> impl Element<Counter> {
    e::button()
        .text(|ctx: RenderCtx<Counter>| *ctx.value)
        .on::<events::Click>(|mut ctx: EventCtx<Counter>, _| {
            *ctx.value += 1;
        })
}
```
> See the [book](https://vivax3794.github.io/natrix/) for more information

## Standout features
* ✅ **No macro DSL** – Macro-based DSLs break formatting & Rust Analyzer support. Natrix avoids them completely for a smoother dev experience.
* ✅ **Callbacks use references to state** – Instead of closures capturing state setters, Natrix callbacks take a reference to the state, which better aligns with Rust’s ownership model.
* ✅ **JS style bundling solution** – Natrix has a compile time css and asset bundling solution that works with dependencies out of the box.

# Design Goals
* **Developer experience first** – Natrix is designed to feel natural for Rust developers.
* **Idiomatic Rust** – We use Rust-native features & patterns, not what worked for js.
* **Stop porting JS to Rust** – Rust is an amazing language, let’s build a frontend framework that actually feels like Rust.
