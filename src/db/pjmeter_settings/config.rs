use crate::io::path;
use anyhow::Result;
use sqlx::{migrate, sqlite::SqlitePoolOptions, Connection, Error, SqliteConnection, SqlitePool};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

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
        conf_path.push("/var/opt/");
        path::panic_if_not_exists(&conf_path);
        conf_path.push("pjmeter");
        path::init_app_config_dir(&conf_path, "Could not create app config directory");
        conf_path.push("config.db");
    } else {
        conf_path.push(env!("CARGO_MANIFEST_DIR"));
        conf_path.push(".sandbox/db");
        path::init_app_config_dir(&conf_path, "Could not create app config directory");
        conf_path.push("global_config.db");
    }

    conf_path
}

async fn update_database(database_url: &str) -> Result<()> {
    let mut conn = SqliteConnection::connect(database_url).await?;
    migrate!("./migrations/pjmeter_settings")
        .run(&mut conn)
        .await
        .map_err(|e| {
            eprintln!("Database update failed: {}", e);
            anyhow::anyhow!("Database update failed: {}", e)
        })?;
    Ok(())
}