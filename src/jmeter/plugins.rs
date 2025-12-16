use crate::project_config::PluginsConfig;
use crate::{io, models};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use users::get_group_by_name;
use std::os::unix::fs::symlink;
use std::time::Duration;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::time::sleep;
use tokio::io::AsyncWriteExt;
use futures_util::StreamExt;
use crate::db::pjmeter_settings;

const JMETER_PLUGIN_PACKAGES: &str = "https://jmeter-plugins.org/files/packages/";
#[cfg(not(debug_assertions))]
const LOCAL_PLUGINS_PATH: &str = "/opt/pjmeter/jmeter_plugins/";

#[cfg(debug_assertions)]
const LOCAL_PLUGINS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.sandbox/jmeter_plugins/");

#[cfg(not(debug_assertions))]
const JMETER_PLUGIN_PATH: &str = "/opt/pjmeter/jmeter/lib/ext/";

#[cfg(debug_assertions)]
const JMETER_PLUGIN_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.sandbox/jmeter/lib/ext/");

#[derive(Debug, Deserialize)]
struct PackageList {
    files: Vec<PluginInfo>,
}

#[derive(Debug, Deserialize)]
pub struct PluginInfo {
    pub file: String,
    pub size: u64,
}

/// Loads all plugins
/// If a plugin isn't available locally : the function tries to download it from https://jmeter-plugins.org/files/packages/
pub async fn load_plugins(plugins: &PluginsConfig) -> Result<()> {
    let mut available_plugins: Vec<models::plugin::Plugin> = Vec::new();
    let mut missing_plugins: HashMap<String, String> = HashMap::new();
    for (name, version) in plugins.plugin_map.iter() {
        match check_if_plugin_is_downloaded(name, version).await {
            Ok(available_plugin) => available_plugins.push(available_plugin),
            Err(_) => { missing_plugins.insert(name.clone(), version.clone()); }
        }
    }

    if !missing_plugins.is_empty() {
        println!("Retrieving missing plugins:");
        let downloadable_plugin_list = fetch_downloadable_plugin_list().await?;
        let total_to_download = missing_plugins.len();
        let mut current_plugin = 1;
        for plugin in missing_plugins.iter() {
            let new_plugin = download_and_register_plugin(
                plugin.0,
                plugin.1,
                &downloadable_plugin_list,
                io::path::get_tmp_dir().as_path(),
                "pjmeter",
                current_plugin,
                total_to_download,
            ).await?;

            available_plugins.push(new_plugin);
            current_plugin += 1;
        }

        println!();
    }

    let available_plugins_path = PathBuf::from(LOCAL_PLUGINS_PATH);
    let jmeter_plugin_path = PathBuf::from(JMETER_PLUGIN_PATH);
    for plugin in available_plugins.iter() {
        for jar in plugin.jars.iter() {
            let target_path = available_plugins_path.join(jar);
            let link_path = jmeter_plugin_path.join(jar);
            match create_plugin_jar_symlink(link_path.as_path(), target_path.as_path()) {
                Ok(_) => {}
                Err(e) => { return Err(anyhow!("Failed to create symlink: {e}")) }
            }
        }
    }

    Ok(())
}

/// Creates a symlink in /opt/pjmeter/jmeter/lib/ext/ to load a plugin
fn create_plugin_jar_symlink(link: impl AsRef<Path>, target: impl AsRef<Path>) -> Result<()> {
    let link = link.as_ref();
    let target = target.as_ref();

    if fs::symlink_metadata(link).is_ok() {
        return Ok(());
    }

    match symlink(target, link) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(anyhow!("Unable to create a link to jar file. Target : {}. Link : {} {e}", target.display(), link.display())),
    }
}

/// Removes all symlinks in /opt/pjmeter/jmeter/lib/ext/
pub fn remove_all_plugin_jars_symlinks() -> Result<u64> {
    let dir = PathBuf::from(JMETER_PLUGIN_PATH);
    let mut removed = 0u64;

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == ErrorKind::NotFound => continue, // raced with deletion
            Err(e) => return Err(anyhow!(e)),
        };

        if meta.file_type().is_symlink() {
            match fs::remove_file(&path) {
                Ok(_) => removed += 1,
                Err(e) if e.kind() == ErrorKind::NotFound => {} // raced with deletion
                Err(e) => return Err(anyhow!(e)),
            }
        }
    }

    Ok(removed)
}

async fn check_if_plugin_is_downloaded(plugin_name: &str, plugin_version: &str) -> Result<models::plugin::Plugin> {
    Ok(pjmeter_settings::settings::get_plugin(pjmeter_settings::config::db_pool(), plugin_name, plugin_version).await?)
}

/// Lists all available plugins
async fn fetch_downloadable_plugin_list() -> Result<Vec<PluginInfo>> {
    let url = JMETER_PLUGIN_PACKAGES;
    let client = Client::new();
    let resp = client.get(url).send().await?.error_for_status()?;
    let package_list: PackageList = resp.json().await?;
    Ok(package_list.files)
}

/// Downloads a plugin file, extracts the jar and registers the plugin in the database.
async fn download_and_register_plugin(
    plugin_name: &str,
    plugin_version: &str,
    downloadable_plugin_list: &[PluginInfo],
    download_dir: &Path,
    group_name: &str,
    current_plugin_number: usize,
    total_to_download: usize,
) -> Result<models::plugin::Plugin> {
    // Check availability first
    let Some(plugin) = find_plugin(downloadable_plugin_list, plugin_name, plugin_version) else {
        println!("  [{current_plugin_number}/{total_to_download}] Plugin {plugin_name}-{plugin_version} : {}", style("NOT FOUND").red().bold());
        return Err(anyhow!(
            "Plugin {} (version {:?}) not found",
            plugin_name,
            plugin_version
        ));
    };

    // Bytes-based progress bar for the download (0% -> 100%)
    let total_bytes = plugin.size;
    let pb = ProgressBar::new(total_bytes);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} {msg} [{bar:40.cyan/blue}] {percent:>3}% \
             {bytes}/{total_bytes}",
        )?.progress_chars("#>-"),
    );
    pb.enable_steady_tick(Duration::from_millis(80));

    let zip_url = format!("{JMETER_PLUGIN_PACKAGES}{}", plugin.file);
    let zip_path = download_dir.join(&plugin.file);
    pb.set_message(format!("{plugin_name}-{plugin_version}"));

    // Monitor the size of the destination file on disk while the download runs
    let (tx_stop, mut rx_stop) = tokio::sync::oneshot::channel::<()>();
    let pb_watch = pb.clone();
    let zip_path_watch = zip_path.clone();

    let watch_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut rx_stop => {
                    // Ensure we render as complete at the end
                    if let Ok(meta) = tokio::fs::metadata(&zip_path_watch).await {
                        let sz = meta.len();
                        pb_watch.set_position(sz.min(total_bytes));
                    }
                    break;
                }
                _ = sleep(Duration::from_millis(100)) => {
                    let sz = tokio::fs::metadata(&zip_path_watch)
                        .await
                        .map(|m| m.len())
                        .unwrap_or(0);
                    pb_watch.set_position(sz.min(total_bytes));
                    // If the observed size already reached/ exceeded the total, we can stop polling.
                    if sz >= total_bytes {
                        break;
                    }
                }
            }
        }
    });

    // Perform the download
    if let Err(e) = download_plugin_zip(&zip_url, &zip_path).await {
        // Stop the watcher and clear the bar before returning the error
        let _ = tx_stop.send(());
        let _ = watch_handle.await;
        pb.finish_and_clear();
        return Err(e);
    }

    // Signal the watcher to stop and finish the bar at 100%
    let _ = tx_stop.send(());
    let _ = watch_handle.await;
    pb.set_position(total_bytes);
    pb.finish_and_clear();

    // Switch to a spinner for subsequent stages
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg} : {prefix} ({elapsed_precise})")?
    );
    spinner.enable_steady_tick(Duration::from_millis(80));

    spinner.set_message(format!("Plugin {plugin_name}-{plugin_version}"));

    // Extracting
    spinner.set_prefix(format!("{}", style("EXTRACTING").cyan().bold()));
    let extracted_jars_paths = extract_jar_from_zip(&zip_path, Path::new(LOCAL_PLUGINS_PATH))?;
    let _ = fs::remove_file(&zip_path);

    // Applying rights
    spinner.set_prefix(format!("{}", style("APPLYING RIGHTS").cyan().bold()));
    for path in &extracted_jars_paths {
        change_group(path, group_name)?;
    }

    // Registering in DB
    spinner.set_prefix(format!("{}", style("REGISTERING").cyan().bold()));
    let mut jar_file_names: Vec<String> = Vec::new();
    for path in &extracted_jars_paths {
        if let Some(name) = path.file_name() {
            jar_file_names.push(name.to_string_lossy().into_owned());
        }
    }

    let pool = pjmeter_settings::config::db_pool();
    pjmeter_settings::settings::add_plugin(pool, plugin_name, plugin_version, jar_file_names).await?;
    let plugin_row = pjmeter_settings::settings::get_plugin(pool, plugin_name, plugin_version).await?;

    // Done
    spinner.finish_and_clear();
    println!("  [{current_plugin_number}/{total_to_download}] Plugin {plugin_name}-{plugin_version} : {}", format!("{}", style("OK").green().bold()));

    Ok(plugin_row)
}

/// Seeks the plugin in the list of available plugins on https://jmeter-plugins.org/files/packages/
/// To find the plugin, the file name must start with the plugin name, contain the version and end with .zip
fn find_plugin<'a>(
    plugins: &'a [PluginInfo],
    plugin_name: &str,
    plugin_version: &str,
) -> Option<&'a PluginInfo> {
    plugins.iter().find(|plugin| {
        plugin.file.starts_with(plugin_name)
        && plugin.file.contains(plugin_version)
        && plugin.file.ends_with(".zip")
    })
}

async fn download_plugin_zip(url: &str, dest: &Path) -> Result<()> {
    let client = Client::new();
    let resp = client.get(url).send().await?.error_for_status()?;

    let mut file = tokio::fs::File::create(dest).await?;
    let mut stream = resp.bytes_stream();

    // Write chunks as they arrive; memory stays bounded to a few chunks
    let mut since_flush = 0usize;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        since_flush += chunk.len();
        if since_flush >= 1 << 20 {
            let _ = file.flush().await; // make growth visible to metadata() for your watcher
            since_flush = 0;
        }
    }
    let _ = file.flush().await;
    Ok(())
}
fn extract_jar_from_zip(zip_path: &Path, dest_dir: &Path) -> Result<Vec<PathBuf>> {
    if !dest_dir.exists() {
        fs::create_dir_all(dest_dir)?;
    } else if !dest_dir.is_dir() {
        return Err(anyhow!("Not a directory: {}", dest_dir.display()));
    }

    let zip_file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(zip_file)?;
    let mut extracted = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;

        if entry.name().ends_with('/') {
            continue;
        }

        // Sécurise le chemin
        let enclosed = match entry.enclosed_name() {
            Some(p) => p.to_owned(),
            None => continue,
        };

        if enclosed.extension().map_or(false, |e| e == "jar") {
            let file_name = match enclosed.file_name() {
                Some(f) => f,
                None => continue,
            };

            let out_path = dest_dir.join(file_name);

            if out_path.exists() {
                extracted.push(out_path);
                continue;
            }

            let mut outfile = File::create(&out_path)?;
            std::io::copy(&mut entry, &mut outfile)?;

            extracted.push(out_path);
        }
    }

    if extracted.is_empty() {
        Err(anyhow!("No .jar file to extract from {}", zip_path.display()))
    } else {
        Ok(extracted)
    }
}

fn change_group(path: &Path, group_name: &str) -> Result<()> {
    let _ = get_group_by_name(group_name)
        .ok_or_else(|| anyhow!("Group {} not found", group_name))?;
    let path_str = path.to_str().ok_or(anyhow!("Invalid jar file path"))?;
    let status = Command::new("chown")
        .arg(format!(":{}", group_name))
        .arg(path_str)
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => Err(anyhow!("Unable to change plugin's group (missing rights)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    // Helper: mock plugin list
    fn mock_plugin_list() -> Vec<PluginInfo> {
        vec![
            PluginInfo { file: "jpgc-cmdrunner-2.3.zip".to_string(), size: 13_000_155 },
            PluginInfo { file: "jpgc-udf-0.6.zip".to_string(), size: 153_025 },
        ]
    }

    #[test]
    fn test_find_plugin_by_name() {
        let plugins = mock_plugin_list();
        assert!(find_plugin(&plugins, "jpgc-cmdrunner", "2.3").is_some());
        assert!(find_plugin(&plugins, "jpgc-udf", "0.6").is_some());
        assert!(find_plugin(&plugins, "jpgc-nonexistent", "2.3").is_none());
    }

    #[test]
    fn test_find_plugin_by_name_and_version() {
        let plugins = mock_plugin_list();
        assert!(find_plugin(&plugins, "jpgc-cmdrunner", "2.3").is_some());
        assert!(find_plugin(&plugins, "jpgc-cmdrunner", "1.9").is_none());
    }

    #[test]
    fn test_download_plugin_zip_and_extract_jar() {
        let tmp = tempdir().unwrap();
        let zip_path = tmp.path().join("test.zip");
        let jar_path = tmp.path().join("test.jar");

        // Crée un zip contenant un fichier jar
        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::FileOptions::<()>::default();
            zip.start_file("test.jar", options).unwrap();
            zip.write_all(b"dummy jar content").unwrap();
            zip.finish().unwrap();
        }

        // Test extraction
        let extracted_paths = extract_jar_from_zip(&zip_path, tmp.path()).unwrap();
        for extracted in extracted_paths {
            assert_eq!(extracted, jar_path);
            let content = fs::read_to_string(&extracted).unwrap();
            assert_eq!(content, "dummy jar content");
        }
}

    #[test]
    fn test_extract_jar_from_zip_with_no_jar() {
        let tmp = tempdir().unwrap();
        let zip_path = tmp.path().join("nojar.zip");

        // Crée un zip sans jar
        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::FileOptions::<()>::default();
            zip.start_file("readme.txt", options).unwrap();
            zip.write_all(b"not a jar").unwrap();
            zip.finish().unwrap();
        }

        let result = extract_jar_from_zip(&zip_path, tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_change_group_fail_invalid_group() {
        let tmp = tempdir().unwrap();
        let jar_path = tmp.path().join("some.jar");
        fs::write(&jar_path, "content").unwrap();

        let result = change_group(&jar_path, "group-that-does-not-exist");
        assert!(result.is_err());
    }

    // Attention : ne teste pas le changement de groupe "réel" car cela nécessite des droits système.
    // Pour un test d'intégration, il faudrait simuler ou mocker l'appel système.

    #[tokio::test]
    async fn test_ensure_plugin_available_plugin_absent() {
        let plugins = mock_plugin_list();
        let tmp = tempdir().unwrap();
        let result = download_and_register_plugin(
            "nonexistent-plugin",
            "None",
            &plugins,
            tmp.path(),
            "my-app-group",
            1,
            1,
        );
        assert!(result.await.is_err());
    }

}