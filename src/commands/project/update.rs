use crate::db::projects_settings;
use crate::io;
use crate::prompts;
use std::path::Path;

pub async fn exec(
    label: Option<&String>,
    new_name: &Option<String>,
    path: &Option<String>,
) -> anyhow::Result<()> {
    let label = match label {
        Some(label) => label.clone(),
        None => prompts::prompt_project_selector().await?,
    };

    let new_name = Some(match new_name {
        Some(new_name) => new_name.clone(),
        None => prompts::get_project_label_from_prompt().await?,
    });

    let path = Some(match path {
        Some(path) => path.clone(),
        None => prompts::get_project_path_from_prompt().await,
    });

    if new_name.is_none() && path.is_none() {
        return Err(anyhow::anyhow!(
            "A new name and/or a new path need to be provided to update the project"
        ));
    }

    let pool = projects_settings::config::db_pool();
    match projects_settings::settings::get_project(pool, &label)
        .await?
        .as_mut()
    {
        Some(updated_project) => {
            if let Some(new_label) = &new_name {
                updated_project.label = new_label.clone();
            }
            if let Some(new_path) = &path {
                let normalised_path = io::path::validate_and_normalize_path(new_path)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;

                if Path::new(&normalised_path).exists() {
                    updated_project.absolute_path = normalised_path;
                } else {
                    return Err(anyhow::anyhow!(
                        "New path doesn't exist: {}",
                        normalised_path
                    ));
                }
            }

            match projects_settings::settings::update_project(pool, &label, updated_project).await {
                Ok(_) => println!("Updated project successfully."),
                Err(e) => eprintln!("An error occurred when updating the project {e}"),
            }
        }
        None => {
            println!("No project with name {label} has been found.");
            println!("Use 'pjmeter project list' to get the list of available projects");
        }
    }

    Ok(())
}
