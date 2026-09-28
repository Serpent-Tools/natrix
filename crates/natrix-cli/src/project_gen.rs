//! Generate project

use std::fs;
use std::path::{Path, PathBuf};

use crate::prelude::*;

/// Flag for extracting css
pub const FEATURE_BUNDLE: &str = "__natrix_internal_bundle";

/// Flag for runtime css
pub const FEATURE_NO_SGG: &str = "__natrix_internal_no_ssg";

/// Generate a new project
pub fn generate_project(name: &str, stable: bool) -> Result<()> {
    let root = PathBuf::from(name);
    fs::create_dir_all(&root)?;

    let src = root.join("src");
    fs::create_dir_all(&src)?;

    let nightly = !stable;

    generate_cargo_toml(name, &root)?;
    generate_main_rs(&src, name)?;
    generate_toolchain_toml(&root, nightly)?;

    let gitignore = "
target
dist
    "
    .trim();
    fs::write(root.join(".gitignore"), gitignore)?;

    let result = std::process::Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(&root)
        .status();
    if let Err(err) = result {
        uwuln!("Failed to init git repo", red.bold);
        uwuln!(err, red.bold);
    }

    print!("✨ ");
    uwu!("Project created ", bright_green);
    uwu!(root.display(), cyan);
    println!();
    uwuln!("Run `natrix dev` to start the dev server", bright_blue);

    Ok(())
}

/// Generate the `cargo.toml`
fn generate_cargo_toml(name: &str, root: &Path) -> Result<(), anyhow::Error> {
    let natrix_version = env!("CARGO_PKG_VERSION");
    let mut natrix_table = format!(r#"version = "{natrix_version}""#);
    if let Ok(path) = std::env::var("NATRIX_PATH") {
        natrix_table = format!(r#"{natrix_table}, path = "{path}""#);
    }
    let natrix_test_table = format!(r#"natrix = {{{natrix_table}, features=["test_utils"]}}"#);
    let features = vec!["default_app"];
    let features = features
        .into_iter()
        .map(|feat| format!(r#""{feat}""#))
        .collect::<Vec<_>>()
        .join(",");
    natrix_table = format!("{natrix_table}, features = [{features}]");
    let natrix_decl = format!("natrix = {{ {natrix_table} }}");
    let natrix_decl = natrix_decl.trim();
    let cargo_toml = format!(
        r#"
[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[dependencies]
{natrix_decl}
log = {{version = "0.4", features = ["max_level_info", "release_max_level_error"]}}

[dev-dependencies]
{natrix_test_table}
wasm-bindgen-test = "0.3.50"

[profile.release]
opt-level = "z"
codegen-units = 1
lto = "fat"
panic = "abort"
strip = "symbols"

[features]
# IMPORTANT: 
# These are feature forwards for internal compilation flags used by the bundler
# DO NOT RENAME OR REMOVE THESE
{FEATURE_BUNDLE} = ["natrix/_internal_bundle"]
{FEATURE_NO_SGG} = ["natrix/_internal_no_ssg"]
        "#
    );
    fs::write(root.join("Cargo.toml"), cargo_toml)?;
    Ok(())
}

/// Generate the main.rs file for a new project
fn generate_main_rs(src: &Path, name: &str) -> Result<(), anyhow::Error> {
    let main_rs = format!(
        r#"
// Panicking in a wasm module will cause the state to be invalid
// And it might cause UB on the next event handler execution.
// (By default natrix uses a panic hook that blocks further event handler calls after a panic)
#![deny(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// These are more strict anti panic lints that you might want to enable
// #![warn(clippy::arithmetic_side_effects, clippy::indexing_slicing, clippy::unreachable)]

use natrix::prelude::*;

const HELLO_ID: Id = natrix::id!();

#[derive(State, Default)]
struct App {{
    counter: Signal<u8>
}}

fn render_app() -> impl Element<App> {{
    e::div()
        .child(e::h1().text("Hello {name}").id(HELLO_ID))
        .child(e::button()
            .text(|ctx: RenderCtx<App>| *ctx.counter) 
            .on::<events::Click>(|mut ctx: EventCtx<App>, _| {{
                *ctx.counter += 1;
            }})
        )
}}

fn main() {{
    natrix::mount(App::default(), render_app);
}}

#[cfg(test)]
mod tests {{
    use super::*;
    use natrix::test_utils;

    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test() {{
        test_utils::mount_test(App::default(), render_app());
        let element = test_utils::get(HELLO_ID);
        assert_eq!(element.text_content(), Some("Hello {name}".to_string()));
    }}
}}
"#
    );
    fs::write(src.join("main.rs"), main_rs)?;

    Ok(())
}

/// Generate the `rust-toolchain.toml`
fn generate_toolchain_toml(root: &Path, nightly: bool) -> Result<(), anyhow::Error> {
    let channel = if nightly { "nightly" } else { "stable" };
    let components = if nightly {
        r#"components = ["rust-src"]"#
    } else {
        ""
    };
    let toolchain_toml = format!(
        r#"
[toolchain]
channel = "{channel}"
targets = ["wasm32-unknown-unknown"]
{components}
        "#
    );
    fs::write(root.join("rust-toolchain.toml"), toolchain_toml)?;
    Ok(())
}
