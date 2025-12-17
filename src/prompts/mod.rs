use crate::db::projects_settings;
use crate::io::path;
use crate::project_config::ProjectConfig;
use anyhow::Result;
use console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, MultiSelect, Select};
use std::path::Path;
use std::process::exit;

pub async fn prompt_display_main_menu(actions: &[&str]) -> Result<usize> {
    let term = Term::stdout();
    let idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose an action")
        .items(actions)
        .default(0)
        .interact_on(&term)?;
    term.clear_last_lines(1)?;

    Ok(idx)
}

pub async fn prompt_project_menu(commands: &[&str]) -> Result<usize> {
    let term = Term::stdout();
    let idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose a project action")
        .items(commands)
        .default(0)
        .interact_on(&term)?;
    term.clear_last_lines(1)?;

    Ok(idx)
}

pub async fn prompt_project_selector() -> Result<String> {
    let pool = projects_settings::config::db_pool();
    let projects = match projects_settings::settings::get_all_projects(pool).await {
        Ok(projects) => projects,
        _ => {
            eprintln!("Failed to get projects from database");
            exit(1);
        }
    };

    let projects: Vec<String> = projects.iter().map(|p| p.label.clone()).collect();
    let term = Term::stdout();
    let idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose a project")
        .items(&projects)
        .default(0)
        .interact_on(&term)?;
    term.clear_last_lines(1)?;

    Ok(projects[idx].clone())
}

pub async fn prompt_environment_selector(project: &String) -> Result<String> {
    let p_config = get_project_config(project).await;

    let environments: Vec<String> = p_config
        .env
        .custom_environments
        .keys()
        .cloned()
        .collect();
    let term = Term::stdout();
    let idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose an environment")
        .items(&environments)
        .default(0)
        .interact_on(&term)?;
    term.clear_last_lines(1)?;

    Ok(environments[idx].clone())
}

pub async fn prompt_project_plans(project: &String) -> Result<Vec<String>> {
    let pool = projects_settings::config::db_pool();
    let project_absolute_path =
        match projects_settings::settings::get_project(pool, project).await {
            Ok(Some(project)) => project.absolute_path,
            _ => {
                eprintln!("Failed to get project {project} from database");
                exit(1);
            }
        };

    let plans = path::list_plan_files(format!("{project_absolute_path}/jmeter/plans").as_str());

    let term = Term::stdout();
    let selection = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose plans to run (select none for all)")
        .items(&plans)
        .interact_on(&term)?;
    term.clear_last_lines(0)?;

    let mut selected_plans = Vec::new();
    for i in selection {
        selected_plans.push(plans[i].clone());
    }

    Ok(selected_plans)
}

async fn get_project_config(project: &String) -> ProjectConfig {
    let pool = projects_settings::config::db_pool();
    match projects_settings::settings::get_project(pool, project).await {
        Ok(Some(some_project)) => ProjectConfig::open(Path::new(&some_project.absolute_path))
            .unwrap_or_else(|e| {
                eprintln!("{e}");
                exit(1)
            }),
        _ => {
            eprintln!("Failed to get project {project} from database");
            exit(1);
        }
    }
}

pub fn prompt_for_input(prompt: &str) -> Result<String> {
    let term = Term::stdout();
    let input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .interact_text_on(&term)?;
    term.clear_last_lines(1)?;

    Ok(input)
}

pub async fn get_project_label_from_prompt() -> String {
    let pool = projects_settings::config::db_pool();
    loop {
        match prompt_for_input("Project's label") {
            Ok(label) => {
                if label.is_empty() {
                    println!("Project label can't be empty");
                    continue;
                }
                match projects_settings::settings::get_project(pool, &label).await {
                    Ok(Some(some_project)) => eprintln!(
                        "A project with the label {label} already exists and points to {}",
                        some_project.absolute_path
                    ),
                    Ok(None) => return label,
                    Err(e) => {
                        eprintln!("Failed to read database {e}");
                        exit(1);
                    }
                }
            }
            _ => continue,
        }
    }
}

pub async fn get_project_path_from_prompt() -> String {
    loop {
        match prompt_for_input("Project path:") {
            Ok(path) => match path::validate_and_normalize_path(&path) {
                Ok(normalized_path) => match path::get_path_if_exists(&normalized_path) {
                    Ok(_) => return path,
                    _ => println!("Invalid path. Enter a valid path."),
                },
                _ => println!("Invalid path. Enter a valid path."),
            },
            _ => continue,
        }
    }
}
