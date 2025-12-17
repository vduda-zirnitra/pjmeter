use crate::io::path;
use anyhow::Result;
use sqlx::{Connection, Error, SqliteConnection, SqlitePool, migrate, sqlite::SqlitePoolOptions};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

static DB_POOL: OnceLock<SqlitePool> = OnceLock::new();

pub async fn init_db_pool() -> Result<()> {
    let conf_path = get_conf_path()?;
    path::init_file(&conf_path).map_err(|e| {
        anyhow::anyhow!(
            "Failed to create database file {}: {}",
            conf_path.display(),
            e
        )
    })?;

    let database_url = format!(
        "sqlite://{}",
        conf_path.to_str().ok_or_else(|| anyhow::anyhow!(
            "Database path contains invalid UTF-8: {}",
            conf_path.display()
        ))?
    );

    // Run your database updates first
    update_database(&database_url).await?;

    // Create the pool
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await
        .map_err(|e| anyhow::anyhow!(
            "Could not connect to the database, check if {} exists and its read/write permissions: {}",
            conf_path.display(), e
        ))?;

    // Store it in the OnceLock
    DB_POOL.set(pool).map_err(|_| Error::PoolClosed)?;

    Ok(())
}

pub fn db_pool() -> &'static SqlitePool {
    DB_POOL
        .get()
        .expect("Database pool not initialized. Call init_db_pool() first.")
}

fn get_conf_path() -> Result<PathBuf> {
    let mut conf_path = PathBuf::new();
    if !cfg!(debug_assertions) {
        // Use platform-specific global/system data directory
        #[cfg(target_os = "windows")]
        {
            // Windows: Use ProgramData (e.g., C:\ProgramData\pjmeter)
            if let Ok(program_data) = std::env::var("PROGRAMDATA") {
                conf_path.push(program_data);
            } else {
                // Fallback to C:\ProgramData
                conf_path.push("C:\\ProgramData");
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: Use /Library/Application Support
            conf_path.push("/Library/Application Support");
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            // Linux and other Unix-like systems: Use /var/opt
            conf_path.push("/var/opt");
        }

        conf_path.push("pjmeter");
        path::init_app_config_dir(&conf_path, "Could not create app config directory")?;
        conf_path.push("config.db");
    } else {
        conf_path.push(env!("CARGO_MANIFEST_DIR"));
        conf_path.push(".sandbox/db");
        path::init_app_config_dir(&conf_path, "Could not create app config directory")?;
        conf_path.push("global_config.db");
    }

    Ok(conf_path)
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
