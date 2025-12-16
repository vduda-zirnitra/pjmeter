use std::fs::{create_dir, File};
use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use std::io::{Write};

pub fn init_jmeter_dir(path: &Path) -> Result<()> {
    let jmeter_dir = create_sub_dir(&path, "jmeter")?;
    let plans_dir = create_sub_dir(&jmeter_dir, "plans")?;
    let results_dir = create_sub_dir(&jmeter_dir, "results")?;

    write_file_to_dir(&jmeter_dir, FileTemplates::RootReadme).context("Failed to create README.md in jmeter/ dir")?;
    write_file_to_dir(&jmeter_dir, FileTemplates::Config).context("Failed to create config.toml in jmeter/ dir")?;
    write_file_to_dir(&jmeter_dir, FileTemplates::Gitignore).context("Failed to create .gitignore in jmeter/ dir")?;

    write_file_to_dir(&plans_dir, FileTemplates::GitKeep).context("Failed to create .gitkeep in jmeter/plans/ dir")?;
    write_file_to_dir(&plans_dir, FileTemplates::SamplePlan).context("Failed to create Sample_plan.jmx in jmeter/plans/ dir")?;

    write_file_to_dir(&results_dir, FileTemplates::GitKeep).context("Failed to create .gitkeep in jmeter/results/ dir")?;

    Ok(())
}

enum FileTemplates {
    RootReadme,
    SamplePlan,
    Config,
    Gitignore,
    GitKeep,
}

fn create_sub_dir(path: &Path, new_dir: &str) -> Result<PathBuf> {
    let mut path_buf = PathBuf::from(&path);
    path_buf.push(new_dir);
    if !path_buf.exists() {
        create_dir(&path_buf).context(format!("Could not create directory {}", path_buf.display()))?;
    }

    Ok(path_buf)
}

fn write_file_to_dir(destination_dir: &Path, template: FileTemplates) -> Result<()> {
    // Embed the file at compile time as a string
    const ROOT_README_CONTENT: &str = include_str!("../../templates/README.md");
    const SAMPLE_PLAN_CONTENT: &str = include_str!("../../templates/Sample_plan.jmx");
    const CONFIG_CONTENT: &str = include_str!("../../templates/config.toml");
    const GITIGNORE_CONTENT: &str = include_str!("../../templates/gitignore_template");
    const GITKEEP_CONTENT: &str = include_str!("../../templates/gitkeep");

    let (content, file_name) = match template {
        FileTemplates::RootReadme => (ROOT_README_CONTENT, "README.md"),
        FileTemplates::GitKeep => (GITKEEP_CONTENT, ".gitkeep"),
        FileTemplates::SamplePlan => (SAMPLE_PLAN_CONTENT, "a_sample_plan.jmx"),
        FileTemplates::Config => (CONFIG_CONTENT, "config.toml"),
        FileTemplates::Gitignore => (GITIGNORE_CONTENT, ".gitignore"),
    };

    // Write the embedded string to the target file
    let dest_path = destination_dir.join(file_name);
    let mut file = File::create(dest_path)?;
    file.write_all(content.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_init_jmeter_dir_creates_structure() {
        let tmp = tempdir().unwrap();
        let result = init_jmeter_dir(tmp.path());
        assert!(result.is_ok());

        // Check directories exist
        assert!(tmp.path().join("jmeter").exists());
        assert!(tmp.path().join("jmeter/plans").exists());
        assert!(tmp.path().join("jmeter/results").exists());

        // Check files exist
        assert!(tmp.path().join("jmeter/README.md").exists());
        assert!(tmp.path().join("jmeter/config.toml").exists());
        assert!(tmp.path().join("jmeter/.gitignore").exists());
        assert!(tmp.path().join("jmeter/plans/.gitkeep").exists());
        assert!(tmp.path().join("jmeter/plans/a_sample_plan.jmx").exists());
        assert!(tmp.path().join("jmeter/results/.gitkeep").exists());
    }

    #[test]
    fn test_init_jmeter_dir_idempotent() {
        let tmp = tempdir().unwrap();
        
        // First call
        let result1 = init_jmeter_dir(tmp.path());
        assert!(result1.is_ok());

        // Second call should not fail
        let result2 = init_jmeter_dir(tmp.path());
        assert!(result2.is_ok());

        // Structure should still be valid
        assert!(tmp.path().join("jmeter").exists());
        assert!(tmp.path().join("jmeter/plans").exists());
        assert!(tmp.path().join("jmeter/results").exists());
    }

    #[test]
    fn test_init_jmeter_dir_files_have_content() {
        let tmp = tempdir().unwrap();
        init_jmeter_dir(tmp.path()).unwrap();

        // Check that files are not empty
        let readme_content = fs::read_to_string(tmp.path().join("jmeter/README.md")).unwrap();
        assert!(!readme_content.is_empty());

        let config_content = fs::read_to_string(tmp.path().join("jmeter/config.toml")).unwrap();
        assert!(!config_content.is_empty());
        assert!(config_content.contains("[jmeter]"));
        assert!(config_content.contains("[plugins]"));
        assert!(config_content.contains("[env."));

        let plan_content = fs::read_to_string(tmp.path().join("jmeter/plans/a_sample_plan.jmx")).unwrap();
        assert!(!plan_content.is_empty());
    }

    #[test]
    fn test_create_sub_dir() {
        let tmp = tempdir().unwrap();
        let result = create_sub_dir(tmp.path(), "test_dir");
        assert!(result.is_ok());
        
        let created_path = result.unwrap();
        assert!(created_path.exists());
        assert!(created_path.is_dir());
        assert_eq!(created_path.file_name().unwrap(), "test_dir");
    }

    #[test]
    fn test_create_sub_dir_already_exists() {
        let tmp = tempdir().unwrap();
        let test_dir = tmp.path().join("test_dir");
        fs::create_dir(&test_dir).unwrap();

        let result = create_sub_dir(tmp.path(), "test_dir");
        assert!(result.is_ok());
        assert!(test_dir.exists());
    }
}