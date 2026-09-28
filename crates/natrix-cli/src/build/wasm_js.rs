//! Build the wasm and js of a file

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::{fs, process};

use oxc::span::GetSpan;

use super::{BINDGEN_OUTPUT_NAME, MACRO_OUTPUT_DIR};
use crate::options::BuildProfile;
use crate::prelude::*;
use crate::project_gen::FEATURE_NO_SGG;
use crate::{options, utils};

/// A renaming map for the wasm-bindgen glue code.
#[derive(Debug)]
pub(crate) struct RenameMap(HashMap<Box<str>, Box<str>>);

/// A visitor to rename ast nodes
struct RenameVisitor<'a> {
    /// The allocator to use
    allocator: &'a oxc::allocator::Allocator,
    /// The resulting mapping.
    mapping: RenameMap,
}

impl<'a> oxc::ast_visit::VisitMut<'a> for RenameVisitor<'a> {
    fn visit_static_member_expression(
        &mut self,
        it: &mut oxc::ast::ast::StaticMemberExpression<'a>,
    ) {
        oxc::ast_visit::walk_mut::walk_static_member_expression(self, it);

        // HACK: This is assuming the names of the exported/imported functions are only used for
        // said exports and imports, which as of writing seems to be the case.
        let current_name = it.property.name.to_string();
        if let Some(new_name) = self.mapping.0.get(&*current_name) {
            let new_name = self.allocator.alloc_str(new_name);

            let current_span = it.property.span;
            let new_identifier = oxc::ast::AstBuilder {
                allocator: self.allocator,
            }
            .identifier_name(current_span, new_name);

            it.property = new_identifier;
        }
    }

    fn visit_property_key(&mut self, it: &mut oxc::ast::ast::PropertyKey<'a>) {
        oxc::ast_visit::walk_mut::walk_property_key(self, it);

        let Some(current_name) = it.static_name() else {
            return;
        };
        if let Some(new_name) = self.mapping.0.get(&*current_name) {
            let new_name = self.allocator.alloc_str(new_name);

            let builder = oxc::ast::AstBuilder {
                allocator: self.allocator,
            };
            let new_identifier = builder.identifier_name(it.span(), new_name);

            *it = oxc::ast::ast::PropertyKey::StaticIdentifier(builder.alloc(new_identifier));
        }
    }
}

/// Run wasmbindgen to generate the glue
pub(crate) fn wasm_bindgen(
    config: &options::BuildConfig,
    wasm_file: &PathBuf,
) -> Result<(PathBuf, PathBuf)> {
    let mut command = process::Command::new("wasm-bindgen");
    command
        .arg(wasm_file)
        .args(["--out-dir", &config.dist.to_string_lossy()])
        .args(["--target", "web"])
        .args(["--out-name", BINDGEN_OUTPUT_NAME])
        .arg("--no-typescript")
        .arg("--omit-default-module-path");
    if config.profile == options::BuildProfile::Dev {
        command.arg("--debug").arg("--keep-debug");
    } else {
        command
            .arg("--remove-name-section")
            .arg("--remove-producers-section")
            .arg("--no-demangle");
    }
    utils::run_with_spinner(command, utils::create_spinner("✍️ wasm_bindgen")?)?;

    let js_file = config.dist.join(format!("{BINDGEN_OUTPUT_NAME}.js"));

    Ok((
        config.dist.join(format!("{BINDGEN_OUTPUT_NAME}_bg.wasm")),
        js_file,
    ))
}

/// Minimize the given js file
pub(crate) fn minimize_js(js_file: &PathBuf, mapping: RenameMap) -> Result<(), anyhow::Error> {
    let spinner = utils::create_spinner("🗜️ Minimizing JS")?;

    let js_code = fs::read_to_string(js_file)?;
    let allocator = oxc::allocator::Allocator::new();
    let parser = oxc::parser::Parser::new(&allocator, &js_code, oxc::span::SourceType::cjs());

    let mut program = parser.parse().program;
    let mut visitor = RenameVisitor {
        allocator: &allocator,
        mapping,
    };
    oxc::ast_visit::walk_mut::walk_program(&mut visitor, &mut program);

    let minifier = oxc::minifier::Minifier::new(oxc::minifier::MinifierOptions {
        mangle: Some(oxc::minifier::MangleOptions {
            top_level: true,
            ..Default::default()
        }),
        compress: Some(oxc::minifier::CompressOptions {
            drop_console: false,
            drop_debugger: true,
            ..Default::default()
        }),
    });
    let symbols = minifier.dce(&allocator, &mut program).scoping;

    let codegen = oxc::codegen::Codegen::new()
        .with_options(oxc::codegen::CodegenOptions {
            minify: true,
            comments: oxc::codegen::CommentOptions::disabled(),
            ..Default::default()
        })
        .with_scoping(symbols);
    let js_code = codegen.build(&program).code;
    std::fs::write(js_file, js_code)?;

    spinner.finish();
    Ok(())
}

/// Build the project wasm
pub(crate) fn build_wasm(config: &options::BuildConfig) -> Result<PathBuf> {
    let rustc_version_meta = rustc_version::version_meta()?;
    let rustc_is_nightly = rustc_version_meta.channel == rustc_version::Channel::Nightly;

    let invalidate = if config.invalidate_cache {
        let time = std::time::SystemTime::now();
        let eleapsed_time = time.duration_since(std::time::UNIX_EPOCH)?;
        eleapsed_time.as_secs()
    } else {
        0
    };

    let settings = natrix_shared::macros::Settings {
        output_dir: config.temp_dir.join(MACRO_OUTPUT_DIR),
        base_path: config.base_path.to_string(),
        invalidate,
    };
    let settings = natrix_shared::macros::serde_json::to_vec(&settings)?;
    let settings = data_encoding::BASE64_NOPAD.encode(&settings);

    let mut command = process::Command::new("cargo");
    command
        .arg("build")
        .args(["--color", "always"])
        .args(["--target", "wasm32-unknown-unknown"])
        .args(["--profile", config.profile.cargo()])
        .env(natrix_shared::MACRO_SETTINGS, settings);

    if config.profile == BuildProfile::Release {
        let mut rustc_flags =
            String::from("-C target-feature=+bulk-memory,+reference-types,+tail-call,+multivalue ");
        if rustc_is_nightly {
            let std_features = String::from("optimize_for_size");
            command
                .args(["-Z", "build-std=std,panic_abort"])
                .arg(format!("-Zbuild-std-features={std_features}"));
            rustc_flags.push_str("-Zfmt-debug=none -Zlocation-detail=none -Zshare-generics=y");
        } else {
            uwuln!(
                    "⚠️ Using stable rust, nightly rust allows for better optimizations and smaller wasm files",
                        yellow.bold
                );
        }
        command.env("RUSTFLAGS", rustc_flags);
    }
    if !config.ssg {
        command.args(["--features", FEATURE_NO_SGG]);
    }

    utils::run_with_spinner(command, utils::create_spinner("⚙️ wasm")?).context("Running cargo")?;

    find_wasm(config).context("Finding wasm file")
}

/// Return the path to the first wasm file in the target folder
pub(crate) fn find_wasm(config: &options::BuildConfig) -> Result<PathBuf> {
    let target = utils::find_target()?;
    let name = utils::get_project_name()?;
    let target = target
        .join("wasm32-unknown-unknown")
        .join(config.profile.target());

    if let Some(wasm) = search_dir_for_wasm(&target, &name)? {
        return Ok(wasm);
    }

    if let Some(wasm) = search_dir_for_wasm(&target.join("deps"), &name)? {
        return Ok(wasm);
    }

    Err(anyhow!("Wasm file not found in {}", target.display()))
}

/// Search the given directory for a wasm file
pub(crate) fn search_dir_for_wasm(
    target: &Path,
    name: &str,
) -> Result<Option<PathBuf>, anyhow::Error> {
    let expected_file_name = format!("{name}.wasm");
    for file in target.read_dir()?.flatten() {
        let path = file.path();
        if let Some(name) = path.file_name() {
            let name = name.to_string_lossy();
            if name == expected_file_name {
                return Ok(Some(file.path()));
            }
        }
    }
    Ok(None)
}

/// Optimize the given wasm file
pub(crate) fn optimize_wasm(wasm_file: &PathBuf) -> Result<RenameMap, anyhow::Error> {
    let spinner = utils::create_spinner("🔎 Optimize wasm")?;

    let mut command = process::Command::new("wasm-opt");
    command
        .arg(wasm_file)
        .arg("-o")
        .arg(wasm_file)
        .arg("--all-features")
        .arg("--strip-debug")
        .arg("--strip-dwarf")
        .arg("--strip-producers")
        .arg("--strip-target-features");

    command.arg("--minify-imports-and-exports-and-modules");

    command.args([
        "--converge",
        "--flatten",
        "--rereloop",
        "--monomorphize",
        "--type-unfinalizing",
        "--generate-global-effects",
        "-Oz",
        "-Oz",
        "--generate-global-effects",
        "--type-finalizing",
        "-Oz",
    ]);

    command.stdout(process::Stdio::piped());

    let mut child = command.spawn()?;
    let stdout = child.stdout.take();
    let result = child.wait()?.success();

    let mut mapping = HashMap::new();

    if let Some(stdout) = stdout {
        let stdout = BufReader::new(stdout);

        let mut seen = HashSet::new();
        'lines_loop: for line in stdout.lines() {
            let line = line?;
            if let Some((old, new)) = line.split_once(" => ") {
                let old = old.trim();
                let new = new.trim();

                if seen.contains(old) {
                    break 'lines_loop;
                }

                // This is not a mistake
                // The wasm-opt output is initially
                // __Wasmbindgen_adjaijda => A
                // ...
                //
                // Once the second optimization pass starts its
                // A => A
                seen.insert(<Box<str>>::from(new));
                mapping.insert(old.into(), new.into());
            }
        }
    }

    // HACK: https://github.com/WebAssembly/binaryen/issues/7657
    // wasm-opt does not report the renaming of the glue module,
    // as of writing it is always renamed to "a" due to being the only module.
    // wasm-bindgen spells that module both as a bare name and as a path to the glue file.
    mapping.insert("wbg".into(), "a".into());
    mapping.insert(format!("./{BINDGEN_OUTPUT_NAME}_bg.js").into(), "a".into());

    spinner.finish();
    if !result {
        return Err(anyhow!(
            "Failed to optimize\nYou might have a outdated wasm-opt version installed\nwe test against v123"
        ));
    }
    Ok(RenameMap(mapping))
}
