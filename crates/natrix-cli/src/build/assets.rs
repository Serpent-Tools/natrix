//! Assets bundling and optimization

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use super::{MACRO_OUTPUT_DIR, options, utils};
use crate::prelude::*;

/// Describes the translation from asset paths to wanted url
#[derive(Default)]
pub struct AssetManifest {
    /// A mapping from runtime url to source path
    pub mapping: HashMap<String, PathBuf>,
}

/// Collect the outputs of the macros
pub fn collect_macro_output(config: &options::BuildConfig) -> Result<AssetManifest> {
    let mut asset_files = Vec::new();

    for file in get_macro_output_files(config)? {
        asset_files.push(file);
    }

    let manifest = collect_asset_manifest(asset_files)?;

    if !config.should_direct_serve_files() {
        copy_assets_to_dist(config, &manifest)?;
    }

    Ok(manifest)
}

/// Copy asset manifest to dist folder
pub fn copy_assets_to_dist(config: &options::BuildConfig, manifest: &AssetManifest) -> Result<()> {
    let spinner = utils::create_spinner("📂 Copying Assets")?;
    for (wanted_url, file) in &manifest.mapping {
        let target_file = config.dist.join(wanted_url);
        if let Err(err) = fs::copy(file, target_file) {
            spinner.finish();
            return Err(err.into());
        }
    }

    spinner.finish();
    Ok(())
}

/// Collect the `.asset` files into a asset manifest
pub fn collect_asset_manifest(asset_files: Vec<PathBuf>) -> Result<AssetManifest> {
    let spinner = utils::create_spinner("📋 Parsing Asset Manifest")?;

    let mut mapping = HashMap::with_capacity(asset_files.len());
    for file in asset_files {
        let mut file_reader = fs::File::open(file)?;
        let natrix_shared::macros::MacroEmission::Asset { path, emitted_path } =
            natrix_shared::macros::serde_json::from_reader(&mut file_reader)?;
        mapping.insert(emitted_path, path);
    }

    spinner.finish();
    Ok(AssetManifest { mapping })
}

/// Get all files in the sub folders of `MACRO_OUTPUT_DIR`
pub fn get_macro_output_files(
    config: &options::BuildConfig,
) -> Result<impl Iterator<Item = PathBuf>> {
    Ok(fs::read_dir(config.temp_dir.join(MACRO_OUTPUT_DIR))?
        .flatten()
        .flat_map(|folder| fs::read_dir(folder.path()).into_iter().flatten().flatten())
        .map(|entry| entry.path()))
}
