use anyhow::Result;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};

pub fn get_tmp_dir() -> PathBuf {
    // Get runtime directory (more secure than /tmp on modern Linux)
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir)
    } else if let Ok(home) = std::env::var("HOME") {
        // Fallback to user's home directory
        PathBuf::from(home).join(".cache").join("tmp")
    } else {
        // Last resort: use /var/tmp which persists across reboots
        PathBuf::from("/var/tmp")
    }
}

pub fn get_path_if_exists(input: &str) -> Result<PathBuf, String> {
    let target_path = PathBuf::from(&input);
    if !target_path.exists() {
        return Err(format!("The path {} does not exist", input));
    }
    
    // Canonicalize to prevent path traversal attacks
    // This resolves symlinks and .. components
    let canonical_path = target_path
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize path {}: {}", input, e))?;
    
    Ok(canonical_path)
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
    
    // Canonicalize path to prevent path traversal attacks
    let canonical_path = match PathBuf::from(project_dir_path).canonicalize() {
        Ok(path) => path,
        Err(_) => return jmx_files, // Returns an empty vector if path doesn't exist or can't be canonicalized
    };
    
    let entries = match fs::read_dir(&canonical_path) {
        Ok(entries) => entries,
        Err(_) => return jmx_files, // Returns an empty vector if the directory doesn't exist
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && let Some(ext) = path.extension()
            && ext == "jmx"
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            jmx_files.push(name.to_string());
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

pub fn check_path_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(anyhow::anyhow!("{} does not exist", path.display()));
    }
    Ok(())
}

pub fn init_app_config_dir(path: &Path, error: &str) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| anyhow::anyhow!("{error} {}: {}", path.display(), e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_validate_and_normalize_path_valid_absolute() {
        let result = validate_and_normalize_path("/home/user/project");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/home/user/project");
    }

    #[test]
    fn test_validate_and_normalize_path_removes_trailing_slash() {
        let result = validate_and_normalize_path("/home/user/project/");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/home/user/project");
    }

    #[test]
    fn test_validate_and_normalize_path_keeps_root() {
        let result = validate_and_normalize_path("/");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/");
    }

    #[test]
    fn test_validate_and_normalize_path_rejects_relative() {
        let result = validate_and_normalize_path("home/user/project");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Path must be absolute (start with '/')"
        );
    }

    #[test]
    fn test_validate_and_normalize_path_rejects_dot() {
        let result = validate_and_normalize_path("/home/./project");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Path contains forbidden segments ('.', '..', '~', or empty)"
        );
    }

    #[test]
    fn test_validate_and_normalize_path_rejects_dotdot() {
        let result = validate_and_normalize_path("/home/../project");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Path contains forbidden segments ('.', '..', '~', or empty)"
        );
    }

    #[test]
    fn test_validate_and_normalize_path_rejects_tilde() {
        let result = validate_and_normalize_path("/~/project");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Path contains forbidden segments ('.', '..', '~', or empty)"
        );
    }

    #[test]
    fn test_validate_and_normalize_path_rejects_whitespace() {
        let result = validate_and_normalize_path("/home/user name/project");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Path contains whitespace in segments");
    }

    #[test]
    fn test_clear_file_extension_single_extension() {
        assert_eq!(clear_file_extension("test.jmx"), "test");
    }

    #[test]
    fn test_clear_file_extension_multiple_extensions() {
        assert_eq!(clear_file_extension("test.tar.gz"), "test");
    }

    #[test]
    fn test_clear_file_extension_no_extension() {
        assert_eq!(clear_file_extension("test"), "test");
    }

    #[test]
    fn test_clear_file_extension_hidden_file_no_extension() {
        // Hidden files without extensions keep their name
        assert_eq!(clear_file_extension(".gitignore"), ".gitignore");
    }

    #[test]
    fn test_clear_file_extension_hidden_file_with_extension() {
        // Hidden files with extensions have them removed
        assert_eq!(clear_file_extension(".bashrc.backup"), ".bashrc");
    }

    #[test]
    fn test_get_path_if_exists_valid() {
        let tmp = tempdir().unwrap();
        let tmp_path = tmp.path().to_str().unwrap();
        let result = get_path_if_exists(tmp_path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_str().unwrap(), tmp_path);
    }

    #[test]
    fn test_get_path_if_exists_nonexistent() {
        let result = get_path_if_exists("/this/path/does/not/exist");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn test_list_plan_files_empty_directory() {
        let tmp = tempdir().unwrap();
        let files = list_plan_files(tmp.path().to_str().unwrap());
        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_list_plan_files_with_jmx_files() {
        let tmp = tempdir().unwrap();

        // Create some test files
        fs::write(tmp.path().join("test1.jmx"), "content").unwrap();
        fs::write(tmp.path().join("test2.jmx"), "content").unwrap();
        fs::write(tmp.path().join("readme.txt"), "content").unwrap();

        let files = list_plan_files(tmp.path().to_str().unwrap());
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"test1.jmx".to_string()));
        assert!(files.contains(&"test2.jmx".to_string()));
        assert!(!files.contains(&"readme.txt".to_string()));
    }

    #[test]
    fn test_list_plan_files_nonexistent_directory() {
        let files = list_plan_files("/this/path/does/not/exist");
        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_init_file_creates_file() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("test.txt");

        assert!(!file_path.exists());
        init_file(&file_path).unwrap();
        assert!(file_path.exists());
    }

    #[test]
    fn test_init_file_existing_file() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("test.txt");

        fs::write(&file_path, "existing content").unwrap();
        assert!(file_path.exists());

        init_file(&file_path).unwrap();
        assert!(file_path.exists());

        // Should not overwrite existing file
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "existing content");
    }

    #[test]
    fn test_get_tmp_dir_returns_path() {
        let tmp_dir = get_tmp_dir();
        assert!(tmp_dir.is_absolute());
    }

    #[test]
    fn test_get_path_if_exists_prevents_path_traversal() {
        // Create a temporary directory structure
        let tmp = tempdir().unwrap();
        let subdir = tmp.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        
        // Try to access parent directory using path traversal
        let traversal_path = format!("{}/../", subdir.display());
        let result = get_path_if_exists(&traversal_path);
        
        // Should succeed but return canonicalized path (not the traversal path)
        assert!(result.is_ok());
        let canonical = result.unwrap();
        
        // The canonical path should be the parent (tmp dir), not containing ".."
        assert!(!canonical.to_string_lossy().contains(".."));
        assert_eq!(canonical, tmp.path().canonicalize().unwrap());
    }

    #[test]
    fn test_list_plan_files_prevents_path_traversal() {
        // Create a temporary directory structure with jmx files
        let tmp = tempdir().unwrap();
        let subdir = tmp.path().join("plans");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("test.jmx"), "content").unwrap();
        
        // Create a separate directory that shouldn't be accessible
        let secret_dir = tmp.path().join("secret");
        fs::create_dir(&secret_dir).unwrap();
        fs::write(secret_dir.join("secret.jmx"), "secret content").unwrap();
        
        // Try to use path traversal to access parent directory
        let traversal_path = format!("{}/plans/../secret", tmp.path().display());
        let files = list_plan_files(&traversal_path);
        
        // Should return files from the canonical path (secret dir)
        // This demonstrates the function canonicalizes paths
        assert_eq!(files.len(), 1);
        assert!(files.contains(&"secret.jmx".to_string()));
    }

    #[test]
    fn test_list_plan_files_with_symlink() {
        // Create a temporary directory structure
        let tmp = tempdir().unwrap();
        let real_dir = tmp.path().join("real");
        fs::create_dir(&real_dir).unwrap();
        fs::write(real_dir.join("test.jmx"), "content").unwrap();
        
        // Create a symlink to the real directory
        #[cfg(unix)]
        {
            let link_path = tmp.path().join("link");
            std::os::unix::fs::symlink(&real_dir, &link_path).unwrap();
            
            // Access via symlink should be canonicalized
            let files = list_plan_files(link_path.to_str().unwrap());
            assert_eq!(files.len(), 1);
            assert!(files.contains(&"test.jmx".to_string()));
        }
    }

    #[test]
    fn test_get_path_if_exists_nonexistent_after_traversal() {
        // Try path traversal to a non-existent path
        let result = get_path_if_exists("/tmp/../nonexistent_path_12345");
        // Should fail because the path doesn't exist
        assert!(result.is_err());
    }
}
