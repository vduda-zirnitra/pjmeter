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