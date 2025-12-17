use crate::models::plugin::Plugin;
use anyhow::Result;
use sqlx::{Error, SqlitePool};

pub async fn add_plugin(
    pool: &SqlitePool,
    name: &str,
    version: &str,
    jars: Vec<String>,
) -> Result<(), Error> {
    let mut jar_ids = Vec::with_capacity(jars.len());
    for file_name in jars {
        let id: i64 = sqlx::query_scalar!(
            "INSERT INTO plugin_jars (file_name)
            VALUES (?)
            ON CONFLICT(file_name) DO UPDATE SET file_name = excluded.file_name
            RETURNING id",
            file_name
        )
        .fetch_one(pool)
        .await?;

        jar_ids.push(id);
    }

    let plugin_id: i64 = sqlx::query_scalar!(
        "INSERT INTO plugins (name, version)
        VALUES (?, ?)
        ON CONFLICT(name, version) DO UPDATE SET name = excluded.name
        RETURNING id",
        name,
        version
    )
    .fetch_one(pool)
    .await?;

    for jar_id in jar_ids {
        let _ = sqlx::query!(
            "INSERT INTO plugins_to_jars (plugin_id, jar_id)
             VALUES (?, ?)
             ON CONFLICT(plugin_id, jar_id) DO NOTHING
             ",
            plugin_id,
            jar_id
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub async fn get_plugin(pool: &SqlitePool, name: &str, version: &str) -> Result<Plugin, Error> {
    let rows = sqlx::query!(
        r#"
        SELECT
          p.name      as "pname: String",
          p.version   as "pversion: String",
          pj.file_name as "jar: Option<String>"
        FROM plugins p
        LEFT JOIN plugins_to_jars p2j ON p2j.plugin_id = p.id
        LEFT JOIN plugin_jars pj       ON pj.id = p2j.jar_id
        WHERE p.name = ? AND p.version = ?
        ORDER BY pj.file_name
        "#,
        name,
        version
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Err(Error::RowNotFound);
    }

    let mut jars = Vec::new();
    for r in &rows {
        if let Some(j) = &r.jar {
            jars.push(j.clone());
        }
    }

    Ok(Plugin {
        _name: rows[0].pname.clone(),
        _version: rows[0].pversion.clone(),
        jars,
    })
}
