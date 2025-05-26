// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

#![expect(rustdoc::invalid_rust_codeblocks)] // Do not interpret code blocks, e.g. license comments.
#![doc = include_str!("../README.md")]

use sqlx::SqlitePool;

mod album_art;
pub use self::album_art::{
    AlbumArt, delete_orphaned_album_art, fetch_album_art, try_load_album_art,
    update_album_art_image,
};

mod information;
pub use self::information::{
    Information, SCHEMA_VERSION_MAJOR, SCHEMA_VERSION_MINOR, fetch_information,
    try_load_information,
};

// TODO: Extract remaining entities into submodules.
mod models;
pub use self::models::*;

#[cfg(feature = "batch")]
mod batch;
#[cfg(feature = "batch")]
pub use self::batch::{BatchOutcome, shrink_album_art};

pub async fn optimize_database(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(r"VACUUM").execute(pool).await?;
    sqlx::query(r"ANALYZE").execute(pool).await?;
    Ok(())
}
