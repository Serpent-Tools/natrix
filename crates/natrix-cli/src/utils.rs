//! Various utility functions

use std::borrow::Cow;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};

use crate::options;
use crate::prelude::*;

/// Create a spinner with the given msg
pub fn create_spinner(msg: &str) -> Result<ProgressBar> {
    let spinner = ProgressBar::new_spinner().with_style(
        ProgressStyle::with_template(&format!(
            "{{spinner:.red}} {} {{msg}}",
            uwu_style!(msg, bright_blue)
        ))?
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏-"),
    );
    spinner.enable_steady_tick(Duration::from_millis(100));
    Ok(spinner)
}

/// Run the given command displaying the given spinner below it
/// And displaying the last line of stderr.
#[expect(
    clippy::needless_pass_by_value,
    reason = "The spinner isnt usable after this"
)]
pub fn run_with_spinner(mut command: process::Command, spinner: ProgressBar) -> Result<String> {
    command
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped());

    let mut child = command.spawn()?;

    let stderr = child.stderr.take().ok_or(anyhow!("Stderr gone"))?;
    let mut stdout = child.stdout.take().ok_or(anyhow!("stdout gone"))?;

    let stderr = BufReader::new(stderr);

    let mut full_output = String::new();
    for line in stderr.lines().map_while(Result::ok) {
        full_output.push_str(&line);
        full_output.push('\n');

        spinner.set_message(line);
    }

    let status = child.wait()?;

    if status.success() {
        spinner.finish_with_message("");

        let mut result = String::new();
        stdout.read_to_string(&mut result)?;

        Ok(result)
    } else {
        spinner.finish_with_message(uwu_style!("ERROR", red).to_string());
        println!("{full_output}");
        Err(anyhow!("Command exited with non zero status"))
    }
}

/// Find if the specified feature is enabled for natrix
#[expect(
    dead_code,
    reason = "No longer used, but might be useful in the future"
)]
pub fn is_feature_enabled(feature: &str, is_default: bool) -> Result<bool> {
    let metadata = cargo_metadata::MetadataCommand::new().no_deps().exec()?;
    let packages = metadata.workspace_default_packages();
    let package = packages.first().ok_or(anyhow!("No package found"))?;
    let natrix = package.dependencies.iter().find(|x| x.name == "natrix");

    Ok(if let Some(natrix) = natrix {
        if natrix.features.iter().any(|feat| feat == feature) {
            true
        } else {
            is_default && natrix.uses_default_features
        }
    } else {
        uwuln!("⚠️ Natrix not found in dependencies", yellow.bold);
        is_default
    })
}

/// Get the natrix version in use
fn natrix_version() -> Result<semver::VersionReq> {
    let metadata = cargo_metadata::MetadataCommand::new().no_deps().exec()?;
    let packages = metadata.workspace_default_packages();
    let package = packages.first().ok_or(anyhow!("No package found"))?;
    let natrix = package
        .dependencies
        .iter()
        .find(|x| x.name == "natrix")
        .ok_or(anyhow!("Natrix now found in dependencies"))?;

    Ok(natrix.req.clone())
}

/// Does the natrix version match the cli version?
pub fn is_natrix_version_matching() -> Result<bool> {
    let natrix_version = natrix_version()?;
    let cli_version = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;

    Ok(natrix_version.matches(&cli_version))
}

/// Find the natrix target folder
pub fn find_target_natrix(mode: options::BuildProfile) -> Result<PathBuf> {
    let target = find_target()?;
    let project = get_project_name()?;
    Ok(target.join(format!("natrix-{project}-{}", mode.readable())))
}

/// Get the current target project name
pub fn get_project_name() -> Result<String> {
    let metadata = cargo_metadata::MetadataCommand::new().no_deps().exec()?;
    let packages = metadata.workspace_default_packages();
    let package = packages.first().ok_or(anyhow!("No package found"))?;

    if packages.len() > 1 {
        return Err(anyhow!(
            "Multiple packages found, please specify the package name"
        ));
    }

    Ok(package.name.to_string())
}

/// Find the target folder
pub fn find_target() -> Result<PathBuf> {
    let metadata = cargo_metadata::MetadataCommand::new().no_deps().exec()?;
    let target = metadata.target_directory;
    let target = PathBuf::from(target);
    Ok(target)
}

/// get the filename of a path
pub fn get_filename(file: &Path) -> Result<Cow<'_, str>> {
    let file_name = file
        .file_name()
        .ok_or(anyhow!("File name not found"))?
        .to_string_lossy();
    Ok(file_name)
}
