use crate::db::projects_settings;
use crate::io;
use crate::jmeter::plugins::{load_plugins, remove_all_plugin_jars_symlinks};
use crate::jmeter::runner;
use crate::project_config::ProjectConfig;
use crate::prompts;
use anyhow::Result;
use std::path::Path;

pub async fn exec(
    project: &Option<String>,
    environment: &Option<String>,
    plans: &Option<Vec<String>>,
) -> Result<()> {
    let project = match project.as_ref().cloned() {
        Some(project) => project,
        None => prompts::prompt_project_selector().await?,
    };

    let environment = match environment.as_ref().cloned() {
        Some(environment) => environment,
        None => prompts::prompt_environment_selector(&project).await?,
    };

    let plans = match plans.as_ref().cloned() {
        Some(plans) => plans,
        None => prompts::prompt_project_plans(&project).await?,
    };

    let pool = projects_settings::config::db_pool();
    let some_project = projects_settings::settings::get_project(pool, &project)
        .await?
        .ok_or_else(|| anyhow::anyhow!(
            "Project {} not found.\nPlease check projects with 'pjmeter project list' or add the project with 'pjmeter project add --label my-project --path /absolute/path/to/my/project/'",
            project
        ))?;

    let p_config = ProjectConfig::open(Path::new(&some_project.absolute_path))
        .map_err(|e| anyhow::anyhow!("Failed to open project config: {}", e))?;

    if !p_config.env.custom_environments.contains_key(&environment) {
        return Err(anyhow::anyhow!(
            "Environment {} not found for project {}\nCheck the environment in {}/jmeter/config.toml",
            environment, project, some_project.absolute_path
        ));
    }

    load_plugins(&p_config.plugins).await?;

    println!("Running plans for project {project} and environment {environment}");
    let plans_to_run = get_plans_to_run(&plans, &some_project.absolute_path);
    runner::run_with_jmeter(
        &p_config,
        &environment,
        plans_to_run,
        &some_project.absolute_path,
    )?;

    let _ = remove_all_plugin_jars_symlinks();

    Ok(())
}

fn get_plans_to_run(plans: &Vec<String>, project_path: &str) -> Vec<String> {
    let project_plans = io::path::list_plan_files(format!("{project_path}/jmeter/plans").as_str());

    if plans.is_empty() || plans[0] == "all" {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_get_plans_to_run_empty_returns_all() {
        let tmp = tempdir().unwrap();
        let plans_dir = tmp.path().join("jmeter/plans");
        fs::create_dir_all(&plans_dir).unwrap();

        // Create test plan files
        fs::write(plans_dir.join("plan1.jmx"), "content").unwrap();
        fs::write(plans_dir.join("plan2.jmx"), "content").unwrap();

        let plans = vec![];
        let result = get_plans_to_run(&plans, tmp.path().to_str().unwrap());

        assert_eq!(result.len(), 2);
        assert!(result.contains(&"plan1.jmx".to_string()));
        assert!(result.contains(&"plan2.jmx".to_string()));
    }

    #[test]
    fn test_get_plans_to_run_all_keyword_returns_all() {
        let tmp = tempdir().unwrap();
        let plans_dir = tmp.path().join("jmeter/plans");
        fs::create_dir_all(&plans_dir).unwrap();

        fs::write(plans_dir.join("plan1.jmx"), "content").unwrap();
        fs::write(plans_dir.join("plan2.jmx"), "content").unwrap();
        fs::write(plans_dir.join("plan3.jmx"), "content").unwrap();

        let plans = vec!["all".to_string()];
        let result = get_plans_to_run(&plans, tmp.path().to_str().unwrap());

        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_get_plans_to_run_specific_valid_plan() {
        let tmp = tempdir().unwrap();
        let plans_dir = tmp.path().join("jmeter/plans");
        fs::create_dir_all(&plans_dir).unwrap();

        fs::write(plans_dir.join("plan1.jmx"), "content").unwrap();
        fs::write(plans_dir.join("plan2.jmx"), "content").unwrap();

        let plans = vec!["plan1.jmx".to_string()];
        let result = get_plans_to_run(&plans, tmp.path().to_str().unwrap());

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "plan1.jmx");
    }

    #[test]
    fn test_get_plans_to_run_multiple_valid_plans() {
        let tmp = tempdir().unwrap();
        let plans_dir = tmp.path().join("jmeter/plans");
        fs::create_dir_all(&plans_dir).unwrap();

        fs::write(plans_dir.join("plan1.jmx"), "content").unwrap();
        fs::write(plans_dir.join("plan2.jmx"), "content").unwrap();
        fs::write(plans_dir.join("plan3.jmx"), "content").unwrap();

        let plans = vec!["plan1.jmx".to_string(), "plan3.jmx".to_string()];
        let result = get_plans_to_run(&plans, tmp.path().to_str().unwrap());

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "plan1.jmx");
        assert_eq!(result[1], "plan3.jmx");
    }

    #[test]
    fn test_get_plans_to_run_invalid_plan_filtered_out() {
        let tmp = tempdir().unwrap();
        let plans_dir = tmp.path().join("jmeter/plans");
        fs::create_dir_all(&plans_dir).unwrap();

        fs::write(plans_dir.join("plan1.jmx"), "content").unwrap();
        fs::write(plans_dir.join("plan2.jmx"), "content").unwrap();

        let plans = vec!["plan1.jmx".to_string(), "nonexistent.jmx".to_string()];
        let result = get_plans_to_run(&plans, tmp.path().to_str().unwrap());

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "plan1.jmx");
    }

    #[test]
    fn test_get_plans_to_run_all_invalid_returns_empty() {
        let tmp = tempdir().unwrap();
        let plans_dir = tmp.path().join("jmeter/plans");
        fs::create_dir_all(&plans_dir).unwrap();

        fs::write(plans_dir.join("plan1.jmx"), "content").unwrap();

        let plans = vec![
            "nonexistent1.jmx".to_string(),
            "nonexistent2.jmx".to_string(),
        ];
        let result = get_plans_to_run(&plans, tmp.path().to_str().unwrap());

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_get_plans_to_run_no_plans_directory() {
        let tmp = tempdir().unwrap();

        let plans = vec!["plan1.jmx".to_string()];
        let result = get_plans_to_run(&plans, tmp.path().to_str().unwrap());

        assert_eq!(result.len(), 0);
    }
}
