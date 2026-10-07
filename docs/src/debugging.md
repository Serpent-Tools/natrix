# Debugging

## Debuggers

When using the `dev` profile natrix will include both DWARF debugging information and a inline sourcemap.

As of writing firefox does not support DWARF based debugging, and in our experience doesnt support wasm breakpoints (including sourcemap backed ones).
Hence we recommend you use chromium for debugging your applications.

> [!TIP]
> Chromium works more than well enough with sourcemap only, but for even better debugging support you can install the [DWARF extension](https://chromewebstore.google.com/detail/cc++-devtools-support-dwa/pdcpmagijalfljmkmjngeonclgbbannb)

## Logging

And ofc a debugging section wont be complete without print-debugging. The default features, specifically `console_log`, setup the [`log`](https://crates.io/crates/log) crate to log to the browser console automatically. you might have already seen some of its output if you open the console in a dev build. You can naturally use the `log` crate yourself to log various information for debugging purposes.

> [!IMPORTANT]
> The default project template sets the log level for dev builds to `info`, you can change this in your `Cargo.toml`
