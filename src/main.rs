mod commands;
mod db;
mod io;
mod jmeter;
mod models;
mod prerequisites;
mod project_config;
mod prompts;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::db::pjmeter_settings;
use crate::db::projects_settings;
use crate::jmeter::plugins::remove_all_plugin_jars_symlinks;
use crate::prerequisites::is_current_user_in_group;

#[derive(Parser)]
#[command(version, author, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Runs JMeter in windowed mode to design test plans
    Design {
        /// The project to work on
        #[arg(short = 'P', long)]
        project: Option<String>,
        /// The environment to work on
        #[arg(short = 'E', long)]
        environment: Option<String>,
    },
    /// Runs JMeter in CLI to execute test plans
    Run {
        /// The project to work on
        #[arg(short = 'P', long)]
        project: Option<String>,
        /// The environment to work on
        #[arg(short = 'E', long)]
        environment: Option<String>,
        /// The list of projects to run
        #[arg(short = 'p', long, value_delimiter = ',')]
        plans: Option<Vec<String>>,
    },
    /// Configure projects with PJmeter
    #[command(subcommand)]
    Project(ProjectCommands),
    /// Checks if everything's OK
    HealthCheck,
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// Add a new project
    Add {
        /// The project's label
        #[arg(short = 'l', long)]
        label: String,
        /// The project's absolute path
        #[arg(short = 'p', long)]
        path: String,
    },
    /// Updates a project
    Update {
        /// The current project's label
        #[arg(short = 'l', long)]
        label: String,
        /// The new project's label
        #[arg(short = 'n', long)]
        new_label: Option<String>,
        /// The new project's path
        #[arg(short = 'p', long)]
        path: Option<String>,
    },
    /// Gives the list of all known projects
    List,
    /// Removes a project
    Remove {
        /// The label of the project to remove
        #[arg(short = 'l', long)]
        label: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    if !is_current_user_in_group("pjmeter")? {
        eprintln!(
            r#"The user running pjmeter doesn't belong to the pjmeter group.
Please add the user to pjmeter group using "usermod -aG pjmeter <user_name>".

To avoid reopening a session while just being added to the group, use "newgrp pjmeter".
"#
        );
    }

    let cli = Cli::parse();
    pjmeter_settings::config::init_db_pool().await?;
    projects_settings::config::init_db_pool().await?;
    let _ = remove_all_plugin_jars_symlinks();

    match &cli.command {
        None => {
            let actions = &["Design", "Run", "Project", "Health Check"];
            match prompts::prompts::prompt_display_main_menu(actions).await? {
                0 => commands::design::exec(&None, &None).await?,
                1 => commands::run::exec(&None, &None, &None).await?,
                2 => handle_project_option().await?,
                3 => commands::healthcheck::exec().await?,
                _ => std::process::exit(0),
            };
        }
        Some(Commands::Design {
            project,
            environment,
        }) => {
            commands::design::exec(project, environment).await?;
        }
        Some(Commands::Run {
            project,
            environment,
            plans,
        }) => {
            commands::run::exec(project, environment, plans).await?;
        }
        Some(Commands::Project(subcmd)) => match subcmd {
            ProjectCommands::Add { label, path } => {
                commands::project::add::exec(Some(label), Some(path)).await?;
            }
            ProjectCommands::Update {
                label,
                new_label: new_name,
                path,
            } => {
                commands::project::update::exec(Some(label), new_name, path).await?;
            }
            ProjectCommands::List => {
                commands::project::list::exec().await?;
            }
            ProjectCommands::Remove { label } => {
                commands::project::remove::exec(Some(label)).await?;
            }
        },
        Some(Commands::HealthCheck) => {
            commands::healthcheck::exec().await?;
        }
    }

    Ok(())
}

async fn handle_project_option() -> Result<()> {
    let project_commands = &["Add", "Update", "List", "Remove"];

    match prompts::prompts::prompt_project_menu(project_commands).await? {
        0 => commands::project::add::exec(None, None).await?,
        1 => commands::project::update::exec(None, &None, &None).await?,
        2 => commands::project::list::exec().await?,
        3 => commands::project::remove::exec(None).await?,
        _ => std::process::exit(0),
    }

    Ok(())
}
