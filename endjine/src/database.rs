// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::path::Path;

use anyhow::{Context as _, bail};
use sqlx::{SqliteExecutor, SqlitePool};

use crate::{DbUuid, Information};

pub async fn open_database(
    file_path: impl AsRef<Path>,
    db_uuid: Option<&DbUuid>,
) -> anyhow::Result<(SqlitePool, Information)> {
    let database_url = format!(
        "sqlite:{file_path}",
        file_path = file_path.as_ref().display()
    );
    let pool = SqlitePool::connect(&database_url).await?;
    let info = if let Some(db_uuid) = db_uuid {
        let Some(info) = Information::try_load_by_uuid(&pool, db_uuid).await? else {
            bail!("found no database information record with UUID {db_uuid}");
        };
        info
    } else {
        Information::load(|| &pool)
            .await
            .context("load database information record")?
    };
    if !info.schema_version().is_supported() {
        bail!(
            "found database {uuid} with unsupported schema version {schema_version}",
            uuid = info.uuid(),
            schema_version = info.schema_version()
        );
    }
    Ok((pool, info))
}

pub async fn optimize_database(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(r"VACUUM").execute(pool).await?;
    sqlx::query(r"ANALYZE").execute(pool).await?;
    Ok(())
}

/// Checks if a table exists in the database.
pub(crate) async fn table_exists(
    executor: impl SqliteExecutor<'_>,
    table_name: &str,
) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM "sqlite_master" WHERE "type"='table' AND "name"=?1)"#,
    )
    .bind(table_name)
    .fetch_one(executor)
    .await
}
