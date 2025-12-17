use crate::db::projects_settings;
use crate::io;
use crate::prompts::prompts;
use anyhow::Result;
use std::process::exit;

pub async fn exec(label: Option<&String>, path: Option<&String>) -> Result<()> {
    let label = match label {
        Some(project) => project.clone(),
        None => prompts::get_project_label_from_prompt().await,
    };
    let path = match path {
        Some(path) => path.clone(),
        None => prompts::get_project_path_from_prompt().await,
    };

    let pool = projects_settings::config::db_pool();
    match projects_settings::settings::get_project(&pool, &label).await? {
        Some(some_project) => eprintln!(
            "A project with the label {label} already exists and points to {}",
            some_project.absolute_path
        ),
        None => {
            let normalised_path =
                io::path::validate_and_normalize_path(&path).unwrap_or_else(|e| {
                    eprintln!("{}", e);
                    exit(1)
                });

            let target_path = io::path::get_path_if_exists(&normalised_path).unwrap_or_else(|e| {
                eprintln!("{}", e);
                exit(1)
            });

            if let Err(e) = io::project_struct::init_jmeter_dir(&target_path) {
                eprintln!("Error: {:?}", e);
                exit(1);
            }

            projects_settings::settings::add_project(&pool, &label, &normalised_path).await?;
            println!("Added project '{label}' with path : '{normalised_path}'");
        }
    }

    Ok(())
}
