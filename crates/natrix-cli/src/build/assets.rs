//! Assets bundling and optimization

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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
    let mut copies = manifest
        .mapping
        .iter()
        .map(|(wanted_url, file)| (file.clone(), config.dist.join(wanted_url)))
        .collect::<Vec<_>>();
    copies.sort_by(|(_, left_target), (_, right_target)| left_target.cmp(right_target));

    let workers = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .min(8);
    copy_files_parallel(&copies, workers, &|source, target| {
        fs::copy(source, target).map(|_| ())
    })?;

    spinner.finish();
    Ok(())
}

/// Copy independent assets concurrently, while joining every worker so failures are reported.
fn copy_files_parallel<F>(
    files: &[(PathBuf, PathBuf)],
    thread_count: usize,
    copy_file: &F,
) -> Result<()>
where
    F: Fn(&Path, &Path) -> std::io::Result<()> + Sync,
{
    if files.is_empty() {
        return Ok(());
    }

    let worker_count = thread_count.max(1).min(files.len());
    let chunk_size = files.len().div_ceil(worker_count);
    std::thread::scope(|scope| {
        let handles = files
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    for (source, target) in chunk {
                        copy_file(source, target)?;
                    }
                    Ok::<(), std::io::Error>(())
                })
            })
            .collect::<Vec<_>>();

        let mut first_error = None;
        for handle in handles {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
                Err(_) => {
                    if first_error.is_none() {
                        first_error = Some(std::io::Error::other("Asset copy task panicked"));
                    }
                }
            }
        }

        if let Some(error) = first_error {
            Err(error.into())
        } else {
            Ok(())
        }
    })
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
