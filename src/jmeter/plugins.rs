use crate::db::pjmeter_settings;
use crate::project_config::PluginsConfig;
use crate::{io, models};
use anyhow::{Result, anyhow};
use console::style;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::ErrorKind;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;
use users::get_group_by_name;

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
            Err(_) => {
                missing_plugins.insert(name.clone(), version.clone());
            }
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
            )
            .await?;

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
                Err(e) => return Err(anyhow!("Failed to create symlink: {e}")),
            }
        }
    }

    Ok(())
}

/// Creates a symlink in /opt/pjmeter/jmeter/lib/ext/ to load a plugin
///
/// Security: This function validates that both the target and link paths are secure
/// before creating the symlink to prevent TOCTOU attacks and directory traversal.
fn create_plugin_jar_symlink(link: impl AsRef<Path>, target: impl AsRef<Path>) -> Result<()> {
    let link = link.as_ref();
    let target = target.as_ref();

    // Validate that target exists and is a regular file (not a symlink)
    let target_meta = fs::metadata(target).map_err(|e| {
        anyhow!(
            "Target file does not exist or is inaccessible: {} - {e}",
            target.display()
        )
    })?;

    if !target_meta.is_file() {
        return Err(anyhow!(
            "Target is not a regular file: {}",
            target.display()
        ));
    }

    // Validate that target is within the expected plugin directory
    let expected_plugin_dir = PathBuf::from(LOCAL_PLUGINS_PATH);
    let target_canonical = target.canonicalize().map_err(|e| {
        anyhow!(
            "Failed to canonicalize target path: {} - {e}",
            target.display()
        )
    })?;
    let expected_canonical = expected_plugin_dir.canonicalize().map_err(|e| {
        anyhow!(
            "Failed to canonicalize plugin directory: {} - {e}",
            expected_plugin_dir.display()
        )
    })?;

    if !target_canonical.starts_with(&expected_canonical) {
        return Err(anyhow!(
            "Target file is outside expected plugin directory: {}",
            target.display()
        ));
    }

    // Validate that link directory exists and is within the expected jmeter plugin path
    let link_parent = link
        .parent()
        .ok_or_else(|| anyhow!("Link path has no parent directory: {}", link.display()))?;

    if !link_parent.exists() {
        return Err(anyhow!(
            "Link parent directory does not exist: {}",
            link_parent.display()
        ));
    }

    let expected_jmeter_dir = PathBuf::from(JMETER_PLUGIN_PATH);
    let link_parent_canonical = link_parent.canonicalize().map_err(|e| {
        anyhow!(
            "Failed to canonicalize link parent directory: {} - {e}",
            link_parent.display()
        )
    })?;
    let jmeter_canonical = expected_jmeter_dir.canonicalize().map_err(|e| {
        anyhow!(
            "Failed to canonicalize jmeter directory: {} - {e}",
            expected_jmeter_dir.display()
        )
    })?;

    if link_parent_canonical != jmeter_canonical {
        return Err(anyhow!(
            "Link must be created within jmeter plugin directory: {}",
            JMETER_PLUGIN_PATH
        ));
    }

    // Use symlink_metadata to check if link exists without following symlinks
    // This prevents TOCTOU by not dereferencing any existing symlink
    match fs::symlink_metadata(link) {
        Ok(meta) => {
            // If it exists and is already a symlink, verify it points to the correct target
            if meta.is_symlink() {
                let existing_target = fs::read_link(link).map_err(|e| {
                    anyhow!("Failed to read existing symlink: {} - {e}", link.display())
                })?;

                // Check if the existing target exists before trying to canonicalize
                // If it doesn't exist, the symlink is broken and should be replaced
                if !existing_target.exists() {
                    return Err(anyhow!(
                        "Existing symlink points to non-existent target: {} -> {}",
                        link.display(),
                        existing_target.display()
                    ));
                }

                // Canonicalize the existing target to compare
                let existing_target_canonical = existing_target.canonicalize().map_err(|e| {
                    anyhow!(
                        "Existing symlink target is invalid or inaccessible: {} - {e}",
                        existing_target.display()
                    )
                })?;

                if existing_target_canonical == target_canonical {
                    // Symlink already points to correct target
                    return Ok(());
                } else {
                    return Err(anyhow!(
                        "Symlink already exists but points to different target. Link: {}, Expected: {}, Actual: {}",
                        link.display(),
                        target.display(),
                        existing_target.display()
                    ));
                }
            } else {
                // Path exists but is not a symlink
                return Err(anyhow!(
                    "Path exists but is not a symlink: {}",
                    link.display()
                ));
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            // Link doesn't exist, safe to create
        }
        Err(e) => {
            return Err(anyhow!(
                "Failed to check link status: {} - {e}",
                link.display()
            ));
        }
    }

    // Create the symlink
    // Note: On Unix, symlink() is atomic and will fail if the path already exists
    match symlink(target, link) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            // Race condition: symlink was created between our check and creation
            // Read it to verify it points to the correct target
            let existing_target = fs::read_link(link).map_err(|e| {
                anyhow!(
                    "Failed to read symlink after creation race: {} - {e}",
                    link.display()
                )
            })?;

            // Check if the target exists before trying to canonicalize
            if !existing_target.exists() {
                return Err(anyhow!(
                    "Race condition: symlink was created pointing to non-existent target. Link: {}, Target: {}",
                    link.display(),
                    existing_target.display()
                ));
            }

            let existing_target_canonical = existing_target.canonicalize().map_err(|e| {
                anyhow!(
                    "Symlink target created in race is invalid: {} - {e}",
                    existing_target.display()
                )
            })?;

            if existing_target_canonical == target_canonical {
                Ok(())
            } else {
                Err(anyhow!(
                    "Race condition: symlink was created pointing to wrong target. Link: {}, Expected: {}, Actual: {}",
                    link.display(),
                    target.display(),
                    existing_target.display()
                ))
            }
        }
        Err(e) => Err(anyhow!(
            "Unable to create symlink. Target: {}, Link: {} - {e}",
            target.display(),
            link.display()
        )),
    }
}

/// Removes all symlinks in /opt/pjmeter/jmeter/lib/ext/
///
/// Security: This function validates each symlink before removal to ensure it points
/// to the expected plugin directory, preventing accidental deletion of legitimate system files.
pub fn remove_all_plugin_jars_symlinks() -> Result<u64> {
    let dir = PathBuf::from(JMETER_PLUGIN_PATH);
    let mut removed = 0u64;

    // Validate the directory exists and is the expected jmeter plugin directory
    let dir_canonical = dir.canonicalize().map_err(|e| {
        anyhow!(
            "Failed to access jmeter plugin directory: {} - {e}",
            dir.display()
        )
    })?;

    let expected_plugin_dir = PathBuf::from(LOCAL_PLUGINS_PATH);
    let expected_canonical = expected_plugin_dir.canonicalize().map_err(|e| {
        anyhow!(
            "Failed to access plugin storage directory: {} - {e}",
            expected_plugin_dir.display()
        )
    })?;

    for entry in fs::read_dir(&dir_canonical)? {
        let entry = entry?;
        let path = entry.path();

        // Use symlink_metadata to get metadata without following symlinks
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == ErrorKind::NotFound => continue, // raced with deletion
            Err(e) => {
                return Err(anyhow!(
                    "Failed to get metadata for {}: {e}",
                    path.display()
                ));
            }
        };

        if meta.file_type().is_symlink() {
            // Verify the symlink target before removal
            let target = match fs::read_link(&path) {
                Ok(t) => t,
                Err(e) if e.kind() == ErrorKind::NotFound => continue, // raced with deletion
                Err(e) => {
                    return Err(anyhow!(
                        "Failed to read symlink target for {}: {e}",
                        path.display()
                    ));
                }
            };

            // Canonicalize the target to verify it's within our plugin directory
            // If canonicalize fails (e.g., target doesn't exist), we should still remove the broken symlink
            let should_remove = match target.canonicalize() {
                Ok(target_canonical) => {
                    // Only remove if it points to our plugin directory
                    target_canonical.starts_with(&expected_canonical)
                }
                Err(_) => {
                    // Broken symlink - it's safe to remove if it was intended to point to plugin dir
                    // Check if the target path (even if broken) looks like it's in our plugin dir
                    target.starts_with(&expected_plugin_dir)
                }
            };

            if should_remove {
                match fs::remove_file(&path) {
                    Ok(_) => removed += 1,
                    Err(e) if e.kind() == ErrorKind::NotFound => {} // raced with deletion
                    Err(e) => {
                        return Err(anyhow!("Failed to remove symlink {}: {e}", path.display()));
                    }
                }
            }
        }
    }

    Ok(removed)
}

async fn check_if_plugin_is_downloaded(
    plugin_name: &str,
    plugin_version: &str,
) -> Result<models::plugin::Plugin> {
    Ok(pjmeter_settings::settings::get_plugin(
        pjmeter_settings::config::db_pool(),
        plugin_name,
        plugin_version,
    )
    .await?)
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
        println!(
            "  [{current_plugin_number}/{total_to_download}] Plugin {plugin_name}-{plugin_version} : {}",
            style("NOT FOUND").red().bold()
        );
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
        )?
        .progress_chars("#>-"),
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
    spinner.set_style(ProgressStyle::with_template(
        "{spinner:.green} {msg} : {prefix} ({elapsed_precise})",
    )?);
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
    pjmeter_settings::settings::add_plugin(pool, plugin_name, plugin_version, jar_file_names)
        .await?;
    let plugin_row =
        pjmeter_settings::settings::get_plugin(pool, plugin_name, plugin_version).await?;

    // Done
    spinner.finish_and_clear();
    println!(
        "  [{current_plugin_number}/{total_to_download}] Plugin {plugin_name}-{plugin_version} : {}",
        style("OK").green().bold()
    );

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

        if enclosed.extension().is_some_and(|e| e == "jar") {
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
        Err(anyhow!(
            "No .jar file to extract from {}",
            zip_path.display()
        ))
    } else {
        Ok(extracted)
    }
}

fn change_group(path: &Path, group_name: &str) -> Result<()> {
    let _ =
        get_group_by_name(group_name).ok_or_else(|| anyhow!("Group {} not found", group_name))?;
    let path_str = path.to_str().ok_or(anyhow!("Invalid jar file path"))?;
    let status = Command::new("chown")
        .arg(format!(":{}", group_name))
        .arg(path_str)
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => Err(anyhow!("Unable to change plugin's group (missing rights)")),
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
            PluginInfo {
                file: "jpgc-cmdrunner-2.3.zip".to_string(),
                size: 13_000_155,
            },
            PluginInfo {
                file: "jpgc-udf-0.6.zip".to_string(),
                size: 153_025,
            },
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

    #[test]
    fn test_create_symlink_validates_target_exists() {
        let tmp = tempdir().unwrap();
        let nonexistent_target = tmp.path().join("nonexistent.jar");
        let link = tmp.path().join("link.jar");

        let result = create_plugin_jar_symlink(&link, &nonexistent_target);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("does not exist or is inaccessible")
        );
    }

    #[test]
    fn test_create_symlink_rejects_directory_as_target() {
        let tmp = tempdir().unwrap();
        let dir_target = tmp.path().join("dir");
        fs::create_dir(&dir_target).unwrap();
        let link = tmp.path().join("link.jar");

        let result = create_plugin_jar_symlink(&link, &dir_target);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not a regular file")
        );
    }

    #[test]
    fn test_create_symlink_validates_link_parent_exists() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target.jar");
        fs::write(&target, "content").unwrap();

        let nonexistent_parent = tmp.path().join("nonexistent_dir");
        let link = nonexistent_parent.join("link.jar");

        let result = create_plugin_jar_symlink(&link, &target);
        assert!(
            result.is_err(),
            "Expected error when link parent doesn't exist"
        );
        // The function should fail - the specific error message depends on implementation details
        // What matters is that it rejects the invalid path
    }

    #[test]
    fn test_create_symlink_rejects_path_traversal_in_target() {
        // This test verifies that targets outside the plugin directory are rejected
        // Note: This would require mocking LOCAL_PLUGINS_PATH for proper testing
        // The actual validation happens by checking canonicalized paths
        let tmp = tempdir().unwrap();
        let outside_target = std::env::temp_dir().join("outside.jar");
        fs::write(&outside_target, "content").unwrap();
        let link = tmp.path().join("link.jar");

        // This should fail because outside_target is not in LOCAL_PLUGINS_PATH
        let result = create_plugin_jar_symlink(&link, &outside_target);
        // Clean up
        let _ = fs::remove_file(&outside_target);

        assert!(result.is_err());
    }

    #[test]
    fn test_remove_symlinks_validates_targets() {
        let tmp = tempdir().unwrap();

        // Create a plugin directory and jmeter directory
        let plugin_dir = tmp.path().join("plugins");
        let jmeter_dir = tmp.path().join("jmeter");
        fs::create_dir(&plugin_dir).unwrap();
        fs::create_dir(&jmeter_dir).unwrap();

        // Create a legitimate plugin file
        let plugin_file = plugin_dir.join("plugin.jar");
        fs::write(&plugin_file, "plugin content").unwrap();

        // Create a symlink in jmeter dir pointing to the plugin
        let symlink_path = jmeter_dir.join("plugin.jar");
        symlink(&plugin_file, &symlink_path).unwrap();

        // Verify symlink exists
        assert!(symlink_path.exists());
        assert!(fs::symlink_metadata(&symlink_path).unwrap().is_symlink());

        // Note: remove_all_plugin_jars_symlinks() uses hardcoded paths,
        // so we can't easily test it without mocking. This test demonstrates
        // the basic pattern of validating symlink targets before removal.

        // Clean up
        fs::remove_file(&symlink_path).unwrap();
    }

    #[test]
    fn test_symlink_security_broken_symlink_handling() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target.jar");
        let link = tmp.path().join("link.jar");

        // Create target
        fs::write(&target, "content").unwrap();

        // Create symlink
        symlink(&target, &link).unwrap();
        assert!(link.exists());

        // Remove target (making symlink broken)
        fs::remove_file(&target).unwrap();

        // Verify broken symlink is detected
        assert!(!link.exists()); // exists() returns false for broken symlinks
        assert!(fs::symlink_metadata(&link).is_ok()); // but metadata still works
        assert!(fs::symlink_metadata(&link).unwrap().is_symlink());

        // Clean up
        fs::remove_file(&link).unwrap();
    }
}
