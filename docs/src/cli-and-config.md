# CLI and Configuration

Natrix comes with a powerful CLI that helps you create, develop, and build your applications. This page explains all the available commands and configuration options.

## The Natrix CLI

The Natrix CLI is a set of tools designed to make working with Natrix projects as smooth as possible. It handles everything from project creation to development servers with hot reloading and optimized production builds.

### Creating New Projects

The `new` command creates a brand new Natrix project with all the necessary files and configuration set up for you.

```bash
natrix new my-awesome-app
```

> [!TIP]
> By default, new projects use nightly Rust for better optimizations and smaller binaries. 
> If you prefer to use stable Rust instead, add the `--stable` flag:
> ```bash
> natrix new my-awesome-app --stable
> ```

This command creates a new project directory with the following structure:

```plain
my-awesome-app/
├── Cargo.toml
├── .gitignore
├── rust-toolchain.toml
└── src/
    └── main.rs
```

### Development Server

The `dev` command starts a local development server with live reloading.

```bash
natrix dev
```

- `--port`/`-p`: The port to serve on, defaults to `8000`, or a random free port if that is taken.
- `--allow-external`: Bind to `0.0.0.0` instead of `127.0.0.1`.
- `--no-reload`: Disable live reloading.

### Building for Production

When you're ready to deploy your app, use the `build` command to create an optimized production build.

```bash
natrix build
```

- `--dist`/`-d`: The output folder, defaults to `./dist`.

Both `dev` and `build` also accept:

- `--profile`: `dev` or `release`, defaults to `dev` for `natrix dev` and `release` for `natrix build`.
- `--invalidate-cache`: Invalidate the asset caches.

## Configuration

Natrix can be configured through your project's `Cargo.toml` file. Add a `[package.metadata.natrix]` section to customize how Natrix builds your application.

>[!IMPORTANT]
> These options only take effect for production builds. For dev all these settings have sensible defaults.

### Cache Busting

Control how asset URLs are versioned to ensure browsers load the latest versions:

```toml
[package.metadata.natrix]
cache_bust = "content"  # Options: "none", "content", "timestamp"
```

- `content`: (Default) Creates a hash based on the file content
- `timestamp`: Creates a hash based on the current build time
- `none`: Doesn't add any cache busting

> [!TIP]
> Content-based cache busting is recommended for production as it only changes URLs when the content actually changes, maximizing cache efficiency.

### Base Path

If your app isn't hosted at the root of a domain, you can specify a base path prefix:

```toml
[package.metadata.natrix]
base_path = "/my-app" 
```

This configures all asset URLs to be prefixed with the specified path.

> [!IMPORTANT]
> Always include a leading slash in your `base_path` value.

### SSG 
By default natrix extracts metadata from your application, importantly for this to work your application must call [`mount`](natrix::reactivity::mount::mount), and should not access any browser apis before or after it. 
If your application does not use `mount` you should set this option to `false`.

This will force css to be injected at runtime instead, and more importantly will not attempt to build and call your binary during bundling.
```toml
[package.metadata.natrix]
ssg = false # Default: True
```

for example if you are doing something like this you need to set `ssg = false`
```rust,compile_fail
fn main() {
    let document = web_sys::window().unwrap(); // This will fail during ssg
    // ...
}
```
