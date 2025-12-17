use crate::db::projects_settings;
use crate::prompts;
use anyhow::Result;

pub async fn exec(label: Option<&String>) -> Result<()> {
    let label = match label {
        Some(label) => label.clone(),
        None => prompts::prompt_project_selector().await?,
    };

    match projects_settings::settings::remove_project(projects_settings::config::db_pool(), &label)
        .await
    {
        Ok(_) => {
            println!("Project {label} was removed");
            println!("Note: project's plans, results, configurations weren't altered");
        }
        Err(e) => eprintln!("Project {label} couldn't be remove because: {e}"),
    }

    Ok(())
}
