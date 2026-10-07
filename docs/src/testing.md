# Testing

Testing is a important part of any project. Natrix doesnt have a dedicated testing framework, instead we recommend you use [wasm-pack](https://rustwasm.github.io/wasm-pack/) to run your tests.
But natrix does provide the [`test_utils`](natrix::test_utils) module to help with testing, which is enabled with the `test_utils` feature flag.

The primary functions are [`mount_test`](natrix::test_utils::mount_test) and [`get`](natrix::test_utils::get).

## Example

```rust
# extern crate natrix;
# extern crate wasm_bindgen_test;
use natrix::prelude::*;

const HELLO: Id = natrix::id!();

#[derive(State)]
struct HelloWorld;

fn render_hello_world() -> impl Element<HelloWorld> {
    e::div()
        .text("Hello World")
        .id(HELLO)
}

mod tests {
    use super::*;
    use natrix::test_utils;
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_hello_world() {
        test_utils::mount_test(HelloWorld, render_hello_world());
        let hello = test_utils::get(HELLO);
        assert_eq!(hello.text_content(), Some("Hello World".to_string()));
    }
}

# fn main() {}
```

This will mount the `render_hello_world` element and then check if the text content of the element with id `HELLO` is "Hello World". This is a simple test, but it shows how to use the `test_utils` module to test your components.
These tests can be run as follows:

```bash
wasm-pack test --headless --chrome --firefox
```

> [!NOTE]
> From our experience the firefox webdriver is very slow to spin up, and even fails at seemingly random times.
