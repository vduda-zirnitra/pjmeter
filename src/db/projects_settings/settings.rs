use crate::models::project::Project;
use anyhow::Result;
use sqlx::sqlite::SqliteQueryResult;
use sqlx::{Error, SqlitePool};

pub async fn add_project(
    pool: &SqlitePool,
    label: &String,
    absolute_path: &String,
) -> Result<SqliteQueryResult, Error> {
    Ok(sqlx::query!(
        "INSERT INTO projects (label, absolute_path) VALUES (?, ?)",
        label,
        absolute_path
    )
    .execute(pool)
    .await?)
}

pub async fn update_project(
    pool: &SqlitePool,
    label: &String,
    project: &Project,
) -> Result<SqliteQueryResult, Error> {
    Ok(sqlx::query!(
            "UPDATE projects SET label = ?, absolute_path = ?, updated_at = CURRENT_TIMESTAMP WHERE label = ?",
            project.label,
            project.absolute_path,
            label
        )
        .execute(pool)
        .await?)
}

pub async fn remove_project(pool: &SqlitePool, label: &String) -> Result<SqliteQueryResult, Error> {
    Ok(sqlx::query!("DELETE FROM projects WHERE label = ?", label)
        .execute(pool)
        .await?)
}

pub async fn get_project(pool: &SqlitePool, label: &String) -> Result<Option<Project>, Error> {
    sqlx::query_as!(
        Project,
        "SELECT label, absolute_path, created_at, updated_at FROM projects WHERE label = ?",
        label
    )
    .fetch_optional(pool)
    .await
    .map_err(|err| err)
}

pub async fn get_all_projects(pool: &SqlitePool) -> Result<Vec<Project>, Error> {
    sqlx::query_as!(
        Project,
        r#"SELECT
              label,
              absolute_path,
              CAST(strftime('%s', created_at) AS INTEGER) AS "created_at!: i64",
              CAST(strftime('%s', updated_at) AS INTEGER) AS "updated_at!: i64"
             FROM projects"#
    )
    .fetch_all(pool)
    .await
    .map_err(|err| err)
}
