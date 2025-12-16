use sqlx::{migrate, sqlite::SqlitePoolOptions, Connection, SqliteConnection, SqlitePool, Error};
use std::path::PathBuf;
use std::time::Duration;
use std::sync::OnceLock;
use anyhow::Result;
use crate::io::path;

static DB_POOL: OnceLock<SqlitePool> = OnceLock::new();

pub async fn init_db_pool() -> Result<()> {
    let conf_path = get_conf_path();
    path::init_file(&conf_path).expect(format!("Failed to create database file {}", conf_path.to_str().unwrap()).as_str());

    let database_url = format!("sqlite://{}", conf_path.to_str().unwrap());

    // Run your database updates first
    update_database(&database_url).await?;

    // Create the pool
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await
        .expect(format!("Could not connect to the database, check if {} exists and its read/write permissions.", conf_path.display()).as_str());

    // Store it in the OnceLock
    DB_POOL.set(pool)
        .map_err(|_| Error::PoolClosed)?;

    Ok(())
}

pub fn db_pool() -> &'static SqlitePool {
    DB_POOL.get().expect("Database pool not initialized. Call init_db_pool() first.")
}

fn get_conf_path() -> PathBuf {
    let mut conf_path = PathBuf::new();
    if !cfg!(debug_assertions) {
        conf_path.push(get_home_dir());
        conf_path.push(".local/share");
        path::panic_if_not_exists(&conf_path);
        conf_path.push("pjmeter");
        path::init_app_config_dir(&conf_path, "Could not create app config directory");
        conf_path.push("config.db");
    } else {
        conf_path.push(env!("CARGO_MANIFEST_DIR"));
        conf_path.push(".sandbox/db");
        path::init_app_config_dir(&conf_path, "Could not create app config directory");
        conf_path.push("local_config.db");
    }

    conf_path
}

fn get_home_dir() -> PathBuf {
    let path = match dirs::home_dir() {
        Some(path) => path,
        None => panic!("Could not find home directory"),
    };

    path
}

async fn update_database(database_url: &str) -> Result<()> {
    let mut conn = SqliteConnection::connect(database_url).await?;
    if let Err(error) = migrate!("./migrations/projects_settings").run(&mut conn).await {
        eprintln!("Database update failed: {}", error);
        return Err(anyhow::anyhow!("Database update failed"));
    }
    Ok(())
}