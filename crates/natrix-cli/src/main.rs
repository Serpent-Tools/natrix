//! Build system and project generator for natrix

use clap::Parser;

/// Reusable imports
mod prelude {
    pub use anyhow::{Context, Result, anyhow};
    pub use owo_colors::OwoColorize;
    pub use owo_colors::Stream::Stdout;
}

/// Helper macro to chain multiple styles and colors from ``owo_colors`` (and println! it), checks if colors are supported.
macro_rules! uwu {
    ($text:expr, $($style:ident).+) => {
        print!(
            "{}",
            $crate::uwu_style!($text, $($style).+)
        );
    };
    ($text:expr) => {
        print!("{}", $text);
    };
}

/// Helper macro to chain multiple styles and colors from ``owo_colors`` (and print! it), checks if colors are supported.
macro_rules! uwuln {
    ($text:expr, $($style:ident).+) => {
        println!(
            "{}",
            $crate::uwu_style!($text, $($style).+)
        );
    };
    ($text:expr) => {
        println!("{}", $text);
    };
}

/// Helper macro to chain multiple styles and colors from ``owo_colors`` (but not print it), checks if colors are supported.
#[macro_export]
macro_rules! uwu_style {
    ($text:expr, $($style:ident).+) => {
        ($text).if_supports_color(Stdout, |string| string$(.$style())+.to_string())
    };
    ($text:expr) => {
        ($text).to_string()
    };
}

use prelude::*;

mod build;
mod dev_server;
mod options;
mod project_gen;
mod utils;

fn main() -> Result<()> {
    let cli = options::Cli::parse();

    match cli {
        options::Cli::New { name, stable } => project_gen::generate_project(&name, stable),
        options::Cli::Dev(args) => dev_server::do_dev(&args),
        options::Cli::Build(args) => {
            build::build(&args.into_build_config()?).context("Building application")?;
            Ok(())
        }
    }
}
