// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use sqlx::SqliteTransaction;

/// ID of the NULL album art that all tracks reference after purging.
const NULL_ALBUM_ART_ID: i64 = 1;

/// Purge album art from all tracks.
///
/// Album art could be restored by re-importing track metadata from files.
///
/// Returns the number of rows deleted from `AlbumArt` (excluding the NULL
/// album art).
pub async fn purge_album_art(tx: &mut SqliteTransaction<'_>) -> sqlx::Result<u64> {
    // Although the albumArtId column is nullable in the database schema
    // Engine DJ chokes up when encountering NULL values. All tracks must
    // at least reference the NULL album art.
    sqlx::query(
        r#"INSERT INTO "AlbumArt" ("id","hash","albumArt") VALUES (?1,NULL,NULL)
           ON CONFLICT ("id") DO UPDATE SET "hash"=NULL,"albumArt"=NULL"#,
    )
    .bind(NULL_ALBUM_ART_ID)
    .execute(&mut **tx)
    .await?;

    // Reset album art of all tracks to the NULL album art. This is required
    // before deleting the referenced album art (ON DELETE RESTRICT).
    sqlx::query(r#"UPDATE "Track" SET "albumArtId"=?1 WHERE "albumArtId" IS NOT ?1"#)
        .bind(NULL_ALBUM_ART_ID)
        .execute(&mut **tx)
        .await?;

    let purged_count = sqlx::query(r#"DELETE FROM "AlbumArt" WHERE "id"<>?1"#)
        .bind(NULL_ALBUM_ART_ID)
        .execute(&mut **tx)
        .await?
        .rows_affected();

    // Reset AUTOINCREMENT primary key sequence of AlbumArt.
    sqlx::query(r#"UPDATE "sqlite_sequence" SET "seq"=?1 WHERE "name"='AlbumArt'"#)
        .bind(NULL_ALBUM_ART_ID)
        .execute(&mut **tx)
        .await?;

    Ok(purged_count)
}
