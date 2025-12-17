use anyhow::Result;
use std::path::Path;
use std::process::exit;

use crate::db::projects_settings;
use crate::jmeter::plugins::{load_plugins, remove_all_plugin_jars_symlinks};
use crate::jmeter::runner;
use crate::project_config::ProjectConfig;
use crate::prompts;

pub async fn exec(project: &Option<String>, environment: &Option<String>) -> Result<()> {
    let project = match project {
        Some(project) => project.clone(),
        None => prompts::prompt_project_selector().await?,
    };
    let environment = match environment {
        Some(environment) => environment.clone(),
        None => prompts::prompt_environment_selector(&project).await?,
    };

    println!("Designing for project {project} and environment {environment}");

    let pool = projects_settings::config::db_pool();
    match projects_settings::settings::get_project(pool, &project).await? {
        None => {
            eprintln!("Project {project} not found.");
            eprintln!(
                "Please check projects with 'pjmeter project list' or add the project with 'pjmeter project add --label my-project --path /absolute/path/to/my/project/"
            );
            exit(1)
        }
        Some(some_project) => {
            let p_config = ProjectConfig::open(Path::new(&some_project.absolute_path))
                .unwrap_or_else(|e| {
                    eprintln!("{e}");
                    exit(1)
                });

            if !p_config.env.custom_environments.contains_key(&environment) {
                eprintln!("Environment {environment} not found for project {project}");
                eprintln!(
                    "Check the environment in {}/jmeter/config.toml",
                    some_project.absolute_path
                );
                exit(1)
            }

            load_plugins(&p_config.plugins).await?;

            runner::design_with_jmeter(&p_config, &environment, &some_project.absolute_path)
                .unwrap_or_else(|e| eprintln!("Failed to run jmeter: {e}"));

            let _ = remove_all_plugin_jars_symlinks();
        }
    }

    Ok(())
}
