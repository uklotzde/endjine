// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use sqlx::SqliteTransaction;

// Engine DJ writes this string into the `albumArt` column.
const DEFAULT_TRACK_ALBUM_ART: &str = "image://planck/0";

/// Resets the album art of all tracks.
///
/// Album art images could be restored by re-importing track metadata from files.
pub async fn reset_album_art(transaction: &mut SqliteTransaction<'_>) -> sqlx::Result<()> {
    sqlx::query(r#"UPDATE "Track" SET "albumArt"=?1,"albumArtId"=NULL"#)
        .bind(DEFAULT_TRACK_ALBUM_ART)
        .execute(&mut **transaction)
        .await?;
    sqlx::query(r#"DELETE FROM "AlbumArt""#)
        .execute(&mut **transaction)
        .await?;
    sqlx::query(r#"UPDATE "sqlite_sequence" SET "seq"=0 WHERE "name"='AlbumArt'"#)
        .execute(&mut **transaction)
        .await?;
    sqlx::query(r#"INSERT INTO "AlbumArt" ("hash","albumArt") VALUES (NULL, NULL)"#)
        .execute(&mut **transaction)
        .await?;
    sqlx::query(r#"UPDATE "Track" SET "albumArtId"=1"#)
        .bind(DEFAULT_TRACK_ALBUM_ART)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}
