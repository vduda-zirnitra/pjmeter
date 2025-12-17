use crate::db::projects_settings;
use tabled::{builder::Builder, settings::Style};

pub async fn exec() -> anyhow::Result<()> {
    let projects =
        projects_settings::settings::get_all_projects(projects_settings::config::db_pool()).await?;

    if projects.is_empty() {
        println!(
            "No project found. You can add one using 'pjmeter project add --label <project_label> --path <project_path>'"
        );
    } else {
        let mut builder = Builder::default();
        builder.push_record(["Label", "Path", "Created at", "Updated at"]);

        for project in projects {
            builder.push_record([
                project.label.clone(),
                project.absolute_path.clone(),
                project.get_created_at().to_rfc3339().clone(),
                project.get_updated_at().to_rfc3339().clone(),
            ]);
        }

        let mut table = builder.build();
        table.with(Style::rounded());
        println!("{table}");
    }

    Ok(())
}
