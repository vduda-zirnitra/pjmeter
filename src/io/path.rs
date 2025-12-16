use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use anyhow::Result;

pub fn get_tmp_dir() -> PathBuf {
    // Get runtime directory (more secure than /tmp on modern Linux)
    let base_dir = if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir)
    } else if let Ok(home) = std::env::var("HOME") {
        // Fallback to user's home directory
        PathBuf::from(home).join(".cache").join("tmp")
    } else {
        // Last resort: use /var/tmp which persists across reboots
        PathBuf::from("/var/tmp")
    };

    base_dir
}

pub fn get_path_if_exists(input: &str) -> Result<PathBuf, String> {
    let target_path = PathBuf::from(&input);
    if !target_path.exists() {
        return Err(format!("The path {} does not exist", input));
    }
    Ok(target_path)
}

pub fn validate_and_normalize_path(input: &str) -> Result<String, &'static str> {
    // 1. Must start with a single /
    if !input.starts_with('/') {
        return Err("Path must be absolute (start with '/')");
    }
    // 2. Forbid forbidden components
    let forbidden = [".", "..", "~"];
    for segment in input.split('/') {
        if forbidden.contains(&segment) {
            return Err("Path contains forbidden segments ('.', '..', '~', or empty)");
        }
        if segment.chars().any(|c| c.is_whitespace()) {
            return Err("Path contains whitespace in segments");
        }
    }
    // 3. Remove trailing '/' if not root
    let normalized = if input != "/" && input.ends_with('/') {
        // Remove trailing slashes
        input.trim_end_matches('/').to_string()
    } else {
        input.to_string()
    };
    Ok(normalized)
}

pub fn list_plan_files(project_dir_path: &str) -> Vec<String> {
    let mut jmx_files = Vec::new();
    let entries = match fs::read_dir(project_dir_path) {
        Ok(entries) => entries,
        Err(_) => return jmx_files, // Retourne un vecteur vide si le dossier n'existe pas
    };

    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "jmx" {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            jmx_files.push(name.to_string());
                        }
                    }
                }
            }
        }
    }
    jmx_files
}

pub fn clear_file_extension(file_name: &str) -> String {
    let mut p = PathBuf::from(file_name);
    while p.extension().is_some() {
        p.set_extension("");
    }
    
    p.to_string_lossy().into_owned()
}

pub fn init_file(conf_path: &PathBuf) -> Result<()> {
    if !conf_path.exists() {
        File::create(conf_path)?;
    }
    Ok(())
}

pub fn panic_if_not_exists(path: &PathBuf) {
    if !path.exists() {
        panic!("{} does not exist", path.to_str().unwrap());
    }
}

pub fn init_app_config_dir(path: &Path, error: &str) {
    match fs::create_dir_all(&path) {
        Ok(_) => (),
        Err(e) => panic!("{error} {}: {}", path.to_str().unwrap(), e),
    }
}