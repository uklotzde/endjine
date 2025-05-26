// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

#![expect(rustdoc::invalid_rust_codeblocks)] // Do not interpret code blocks, e.g. license comments.
#![doc = include_str!("../README.md")]

use sqlx::SqlitePool;

mod models;
pub use self::models::*;

#[cfg(feature = "batch")]
mod batch;
#[cfg(feature = "batch")]
pub use self::batch::{BatchOutcome, shrink_album_art};

pub async fn delete_orphaned_album_art(pool: &SqlitePool) -> sqlx::Result<u64> {
    let result =
        sqlx::query(r"DELETE FROM AlbumArt WHERE id NOT IN (SELECT albumArtId FROM Track)")
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

pub async fn optimize_database(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(r"VACUUM").execute(pool).await?;
    sqlx::query(r"ANALYZE").execute(pool).await?;
    Ok(())
}
