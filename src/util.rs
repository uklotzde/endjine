// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use sqlx::SqlitePool;

pub async fn optimize_database(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(r"VACUUM").execute(pool).await?;
    sqlx::query(r"ANALYZE").execute(pool).await?;
    Ok(())
}
