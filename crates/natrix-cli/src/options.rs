//! Define cli and config options

use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use serde::Deserialize;

use crate::dev_server::get_free_port;
use crate::prelude::*;
use crate::utils;

/// The toml config
#[derive(Deserialize)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct NatrixConfig {
    /// The cache busting strategy
    pub cache_bust: CacheBustOption,
    /// The base url to use
    pub base_path: Box<str>,
    /// Whether ssg should be done
    pub ssg: bool,
}

impl Default for NatrixConfig {
    fn default() -> Self {
        Self {
            cache_bust: CacheBustOption::Content,
            base_path: Box::from(""),
            ssg: true,
        }
    }
}

/// Metadata section
#[derive(Deserialize)]
struct PackageMetadata {
    /// The natrix config
    natrix: Option<NatrixConfig>,
}

impl NatrixConfig {
    /// Read the natrix config from cargo.toml metadata
    pub(crate) fn read_config() -> Result<Self> {
        let cargo_metadata = cargo_metadata::MetadataCommand::default()
            .no_deps()
            .exec()?;
        let packages = cargo_metadata.workspace_default_packages();
        let package = packages.first().ok_or(anyhow!("No package found"))?;

        if package.metadata.is_null() {
            return Ok(Self::default());
        }
        let package_metadata: PackageMetadata = serde_json::from_value(package.metadata.clone())?;

        Ok(package_metadata.natrix.unwrap_or_default())
    }
}

/// Natrix CLI
#[derive(Parser)]
#[clap(version, about, author)]
pub enum Cli {
    /// Create a new project
    New {
        /// The name of the project
        name: String,
        /// Use Stable rust
        #[arg(short, long)]
        stable: bool,
    },
    /// Spawn a dev server
    Dev(DevArguments),
    /// Build the project
    Build(BuildArguments),
}

/// Arguments for the dev subcommand
#[derive(Parser)]
pub struct DevArguments {
    /// Port to use for dev server
    #[arg(short, long)]
    pub port: Option<u16>,
    /// Bind to 0.0.0.0 instead of 127.0.0.1
    #[arg(long)]
    pub allow_external: bool,
    /// Disable live-reloading
    #[arg(long)]
    pub no_reload: bool,
    /// The shared arguments
    #[command(flatten)]
    pub shared: SharedArguments,
}

/// Arguments for the build subcommand
#[derive(Parser)]
pub struct BuildArguments {
    /// The target dist folder
    #[arg(short, long)]
    pub dist: Option<PathBuf>,
    /// The shared arguments
    #[command(flatten)]
    pub shared: SharedArguments,
}

/// Settings for building the server
#[derive(Parser)]
pub struct SharedArguments {
    /// Build profile to use
    #[arg(long, value_enum)]
    pub profile: Option<BuildProfile>,
    /// Invalidate the asset caches
    #[arg(long)]
    pub invalidate_cache: bool,
}

/// Settings for building the server
pub struct BuildConfig {
    /// Build profile to use
    pub profile: BuildProfile,
    /// Location to output build files
    pub dist: PathBuf,
    /// Location for the temp dir
    pub temp_dir: PathBuf,
    /// Do live reload
    /// The Some value is the port to use
    pub live_reload: Option<u16>,
    /// Cache bust option
    pub cache_bust: CacheBustOption,
    /// The base url to use
    pub base_path: Box<str>,
    /// Invalidate the asset caches
    pub invalidate_cache: bool,
    /// Whether to do ssg
    pub ssg: bool,
}

impl DevArguments {
    /// Create a `BuildConfig` from `DevArguments` with appropriate defaults
    pub fn get_build_config(&self) -> Result<BuildConfig> {
        let profile = self.shared.profile.unwrap_or(BuildProfile::Dev);
        let target = utils::find_target_natrix(profile)?;

        let dist = target.join("dist");

        // Always try to use port 9000 for live reload WebSocket
        let live_reload = if self.no_reload {
            None
        } else if let Ok(port) = get_free_port(9000) {
            Some(port)
        } else {
            uwuln!("No free port found for live reload, disabling it", red.bold);
            None
        };

        Ok(BuildConfig {
            profile,
            dist,
            temp_dir: target,
            live_reload,
            cache_bust: CacheBustOption::Timestamp,
            base_path: Box::from(""),
            invalidate_cache: self.shared.invalidate_cache,
            ssg: false,
        })
    }
}

impl BuildArguments {
    /// Convert `BuildArguments` to `BuildConfig` with appropriate defaults
    pub(crate) fn into_build_config(self) -> Result<BuildConfig> {
        let config = NatrixConfig::read_config()?;

        let profile = self.shared.profile.unwrap_or(BuildProfile::Release);
        Ok(BuildConfig {
            profile,
            dist: self.dist.unwrap_or_else(|| PathBuf::from("./dist")),
            temp_dir: utils::find_target_natrix(profile)?,
            live_reload: None,
            cache_bust: config.cache_bust,
            base_path: config.base_path,
            invalidate_cache: self.shared.invalidate_cache,
            ssg: config.ssg && profile == BuildProfile::Release,
        })
    }
}

impl BuildConfig {
    /// Should dev sever do direct serving
    pub fn should_direct_serve_files(&self) -> bool {
        self.profile == BuildProfile::Dev && self.live_reload.is_some()
    }
}

/// Build profile
#[derive(Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum BuildProfile {
    /// Runs with optimizations
    Release,
    /// Does not do any optimization
    Dev,
}

impl BuildProfile {
    /// Return a more readable version of this profile name
    pub fn readable(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Dev => "dev",
        }
    }

    /// Return the cargo profile name
    pub fn cargo(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Dev => "dev",
        }
    }

    /// Return the target output folder
    pub fn target(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Dev => "debug",
        }
    }
}

/// Cache busting options
#[derive(Clone, Copy, ValueEnum, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheBustOption {
    /// No cache busting
    None,
    /// Crate a hash based on the content
    Content,
    /// Create a hash based on the timestamp
    Timestamp,
}
