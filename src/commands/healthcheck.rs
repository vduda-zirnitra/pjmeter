use std::path::Path;
use crate::db::projects_settings;

pub async fn exec() -> anyhow::Result<()> {
    let projects = projects_settings::settings::get_all_projects(projects_settings::config::db_pool()).await?;
    let mut with_error = false;
    for project in projects {
        if !Path::new(&project.absolute_path).exists() {
            with_error = true;
            println!("Path for project {} is broken. Expected but not found : {}", project.label, project.absolute_path);
        }
    }

    println!();
    if with_error {
        println!("Healthcheck finished: fix errors displayed above.");
    } else {
        println!("Healthcheck finished: no errors detected.");
    }
    
    Ok(())
}