use std::path::Path;
use std::process::exit;
use anyhow::Result;
use crate::io;
use crate::db::projects_settings;
use crate::jmeter::plugins::{load_plugins, remove_all_plugin_jars_symlinks};
use crate::jmeter::runner;
use crate::project_config::ProjectConfig;
use crate::prompts::prompts;

pub async fn exec(project: &Option<String>, environment: &Option<String>, plans: &Option<Vec<String>>) -> Result<()> {
    let project = match project.as_ref().cloned() {
        Some(project) => project,
        None => prompts::prompt_project_selector().await?,
    };

    let environment= match environment.as_ref().cloned() {
        Some(environment) => environment,
        None => prompts::prompt_environment_selector(&project).await?,
    };

    let plans= match plans.as_ref().cloned() {
        Some(plans) => plans,
        None => prompts::prompt_project_plans(&project).await?,
    };

    let pool = projects_settings::config::db_pool();
    match projects_settings::settings::get_project(&pool, &project).await? {
        None => {
            eprintln!("Project {project} not found.");
            eprintln!("Please check projects with 'pjmeter project list' or add the project with 'pjmeter project add --label my-project --path /absolute/path/to/my/project/");
            exit(1)
        }
        Some(some_project) => {
            let p_config = ProjectConfig::open(Path::new(&some_project.absolute_path))
                .unwrap_or_else(|e| { eprintln!("{e}"); exit(1) });

            if !p_config.env.custom_environments.contains_key(&environment) {
                eprintln!("Environment {environment} not found for project {project}");
                eprintln!("Check the environment in {}/jmeter/config.toml", some_project.absolute_path);
                exit(1)
            }

            load_plugins(&p_config.plugins).await?;

            println!("Running plans for project {project} and environment {environment}");
            let plans_to_run = get_plans_to_run(&plans, &some_project.absolute_path);
            runner::run_with_jmeter(&p_config, &environment, plans_to_run, &some_project.absolute_path)
                .unwrap_or_else(|e| eprintln!("Failed to run jmeter: {e}"));

            let _ = remove_all_plugin_jars_symlinks();
        }
    }

    Ok(())
}

fn get_plans_to_run(plans: &Vec<String>, project_path: &str) -> Vec<String> {
    let project_plans = io::path::list_plan_files(format!("{project_path}/jmeter/plans").as_str());

    if plans.len() == 0 || plans[0] == "all" {
        project_plans
    } else {
        let mut validated_plans: Vec<String> = Vec::new();
        for plan in plans {
            if !project_plans.contains(plan) {
                eprintln!("Plan {plan} not found.");
            } else {
                validated_plans.push(plan.clone());
            }
        }
        validated_plans
    }
}