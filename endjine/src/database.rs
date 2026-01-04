// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::path::Path;

use sqlx::SqlitePool;

use crate::{DbUuid, Information};

pub async fn open_database(
    file_path: impl AsRef<Path>,
    db_uuid: Option<&DbUuid>,
) -> sqlx::Result<(SqlitePool, Information)> {
    let database_url = format!(
        "sqlite:{file_path}",
        file_path = file_path.as_ref().display()
    );
    let pool = SqlitePool::connect(&database_url).await?;
    let info = if let Some(db_uuid) = &db_uuid {
        if let Some(info) = Information::try_load_by_uuid(&pool, db_uuid).await? {
            info
        } else {
            // TODO: Use a custom error type.
            log::warn!("Found no database information record with UUID {db_uuid}");
            return Err(sqlx::Error::RowNotFound);
        }
    } else {
        let mut info_all = Information::load_all(&pool).await?;
        let info_count = info_all.len();
        let Some(info) = info_all.pop() else {
            // TODO: Use a custom error type.
            log::warn!("Found no database information records");
            return Err(sqlx::Error::RowNotFound);
        };
        // Only a single row is expected.
        if !info_all.is_empty() {
            // TODO: Use a custom error type.
            log::warn!("Found multiple ({info_count}) database information records");
            return Err(sqlx::Error::RowNotFound);
        }
        info
    };
    if !info.schema_version().is_supported() {
        // TODO: Use a custom error type.
        log::error!(
            "Found database {uuid} with unsupported schema version {schema_version}",
            uuid = info.uuid(),
            schema_version = info.schema_version()
        );
        return Err(sqlx::Error::RowNotFound);
    }
    Ok((pool, info))
}

pub async fn optimize_database(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(r"VACUUM").execute(pool).await?;
    sqlx::query(r"ANALYZE").execute(pool).await?;
    Ok(())
}
