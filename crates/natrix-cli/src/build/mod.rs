//! Build a natrix project

use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use crate::prelude::*;
use crate::{options, utils};

pub mod assets;
mod css;
mod sourcemap;
mod wasm_js;
mod wasm_parser;

/// The directory to store macro outputs
const MACRO_OUTPUT_DIR: &str = "macro";
/// The name of the js file
const BINDGEN_OUTPUT_NAME: &str = "code";
/// Name of the collected css
const CSS_OUTPUT_NAME: &str = "styles.css";

/// Build a project
pub fn build(config: &options::BuildConfig) -> Result<assets::AssetManifest> {
    if !utils::is_natrix_version_matching()? {
        uwuln!("Cli version does not match natrix version.", red.bold);
    }

    print!("🧹 ");
    uwuln!("Cleaning dist", bright_black);
    let _ = fs::remove_dir_all(&config.dist);

    if config.invalidate_cache {
        let _ = fs::remove_dir_all(&config.temp_dir);
    }
    let _ = fs::create_dir_all(config.temp_dir.join(MACRO_OUTPUT_DIR));

    print!("🚧 ");
    uwu!("Starting Build ", bright_blue);
    print!("(using profile ");
    uwu!(config.profile.readable(), cyan);
    println!();

    std::fs::create_dir_all(&config.dist).context("Creating dist")?;

    let source_wasm_file = wasm_js::build_wasm(config).context("Building wasm")?;

    // These steps depend on the wasm build's outputs, but not on one another. Run them together
    // so asset copying and native CSS extraction can overlap wasm-bindgen and wasm optimization.
    let (wasm_outputs, asset_manifest, raw_css) = run_parallel(
        || -> Result<_> {
            let (wasm_file, js_file) = wasm_js::wasm_bindgen(config, &source_wasm_file)?;
            if config.profile == options::BuildProfile::Release {
                let rename_map = wasm_js::optimize_wasm(&wasm_file)?;
                wasm_js::minimize_js(&js_file, rename_map)?;
            }

            let wasm_file = cache_bust_file(config, wasm_file)?;
            let js_file = cache_bust_file(config, js_file)?;

            // Parse wasm file once for both sourcemap and CSS optimization.
            let wasm_parse_result = if config.profile == options::BuildProfile::Dev || config.ssg {
                Some(wasm_parser::parse_wasm_file(&wasm_file)?)
            } else {
                None
            };

            if config.profile == options::BuildProfile::Dev {
                uwuln!("🗺️ Generating source map", bright_blue);
                let parse_result = wasm_parse_result
                    .as_ref()
                    .ok_or_else(|| anyhow!("Wasm parse result missing for sourcemap generation"))?;
                sourcemap::create_sourcemap(&wasm_file, parse_result)?;
            }

            Ok((wasm_file, js_file, wasm_parse_result))
        },
        || assets::collect_macro_output(config),
        || {
            if config.ssg {
                css::extract_css().map(Some)
            } else {
                Ok(None)
            }
        },
    )?;

    let (wasm_file, js_file, wasm_parse_result) = wasm_outputs;
    let css_file = if let Some(raw_css) = raw_css {
        let parse_result = wasm_parse_result
            .as_ref()
            .ok_or_else(|| anyhow!("Wasm parse result missing for CSS optimization"))?;
        let css_file = css::write_css(config, parse_result, raw_css)?;
        Some(cache_bust_file(config, css_file)?)
    } else {
        None
    };

    generate_html(config, &wasm_file, &js_file, css_file)?;

    print!("📦 ");
    uwu!("Result in", bright_blue);
    uwuln!(config.dist.display(), cyan);

    Ok(asset_manifest)
}

/// Run three independent build tasks at the same time and preserve task errors.
fn run_parallel<A, B, C>(
    first: impl FnOnce() -> Result<A> + Send,
    second: impl FnOnce() -> Result<B> + Send,
    third: impl FnOnce() -> Result<C> + Send,
) -> Result<(A, B, C)>
where
    A: Send,
    B: Send,
    C: Send,
{
    std::thread::scope(|scope| {
        let first = scope.spawn(first);
        let second = scope.spawn(second);
        let third = scope.spawn(third);

        let first = first
            .join()
            .map_err(|_| anyhow!("Wasm build task panicked"))??;
        let second = second
            .join()
            .map_err(|_| anyhow!("Asset collection task panicked"))??;
        let third = third
            .join()
            .map_err(|_| anyhow!("CSS extraction task panicked"))??;

        Ok((first, second, third))
    })
}

/// Generate the html file to be used
fn generate_html(
    config: &options::BuildConfig,
    wasm_file: &Path,
    js_file: &Path,
    css_file: Option<PathBuf>,
) -> Result<()> {
    let base_path = &config.base_path;

    let html_file = config.dist.join("index.html");
    let js_file = utils::get_filename(js_file)?;
    let wasm_file = utils::get_filename(wasm_file)?;

    let style_link = if let Some(css_file) = css_file {
        let css_file = utils::get_filename(&css_file)?;
        format!(r#"<link rel="stylesheet" href="{base_path}/{css_file}"/>"#)
    } else {
        String::new()
    };

    let js_reload = if let Some(port) = config.live_reload {
        format!(
            "
            const reload_ws = new WebSocket(`ws://${{window.location.hostname}}:{port}`);
            reload_ws.onmessage = (event) => {{
                location.reload();
            }};
            "
        )
    } else {
        String::new()
    };

    let content = format!(
        r#"
<!doctype html>
<html>
    <head>
        {style_link}
        <link rel="preload" as="fetch" href="{base_path}/{wasm_file}" crossorigin="anonymous"/>
        <meta name="viewport" content="width=device-width, initial-scale=1">
    </head>
    <body>
        <noscript>
            <h1 style="color: red">This website requires Javascript and Wasm support.</h1>
        </noscript>
        <div id="{}" style="color: orange">Currently loading</div>
        <script type="module">
            import init from "{base_path}/{js_file}";
            init({{module_or_path:"{base_path}/{wasm_file}"}});
            {js_reload}
        </script>
    </body>
</html>
    "#,
        natrix_shared::MOUNT_POINT,
    );

    std::fs::write(html_file, content.trim())?;

    Ok(())
}

/// Moves the given file to a new location in accordane with cache busting options
/// Returns the new file location
fn cache_bust_file(config: &options::BuildConfig, original_file: PathBuf) -> Result<PathBuf> {
    let Some(original_filename) = original_file.file_name() else {
        return Ok(original_file);
    };
    let original_filename = original_filename.to_string_lossy();

    let new_filename = match config.cache_bust {
        options::CacheBustOption::None => original_filename.into_owned(),
        options::CacheBustOption::Timestamp => {
            let current_time = std::time::SystemTime::now();
            let since_epoch = current_time.duration_since(std::time::UNIX_EPOCH)?;
            let unix_time_stamp = since_epoch.as_secs();
            let encoded_timestamp =
                data_encoding::BASE64URL_NOPAD.encode(&unix_time_stamp.to_le_bytes());
            format!("{encoded_timestamp}-{original_filename}")
        }
        options::CacheBustOption::Content => {
            let content = fs::read(&original_file)?;
            let mut hasher = DefaultHasher::default();
            content.hash(&mut hasher);
            let hash = hasher.finish();

            let encoded_hash = data_encoding::BASE64URL_NOPAD.encode(&hash.to_le_bytes());
            format!("{encoded_hash}-{original_filename}")
        }
    };

    let new_file = original_file.with_file_name(new_filename);

    fs::rename(original_file, &new_file)?;
    Ok(new_file)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::Duration;

    use super::run_parallel;

    #[test]
    fn independent_build_tasks_start_concurrently() {
        let started = Arc::new((Mutex::new(0_usize), Condvar::new()));
        let task = || {
            let started = Arc::clone(&started);
            move || {
                let (count, changed) = &*started;
                let mut count = count
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Start counter was poisoned"))?;
                *count = count.saturating_add(1);
                changed.notify_all();
                let (count, timeout) = changed
                    .wait_timeout_while(count, Duration::from_secs(2), |count| *count < 3)
                    .map_err(|_| anyhow::anyhow!("Start counter was poisoned"))?;
                if timeout.timed_out() {
                    return Err(anyhow::anyhow!("Build tasks did not start concurrently"));
                }
                if *count == 3 {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Build task start count was corrupted"))
                }
            }
        };

        assert!(
            run_parallel(task(), task(), task()).is_ok(),
            "all independent tasks should overlap"
        );
    }
}
