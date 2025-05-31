// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use sqlx::SqliteTransaction;

/// Purge album art from all tracks.
///
/// Album art could be restored by re-importing track metadata from files.
///
/// Returns the number of rows deleted from `AlbumArt` (excluding the NULL
/// album art).
pub async fn purge_album_art(tx: &mut SqliteTransaction<'_>) -> sqlx::Result<u64> {
    // Reset album art of all tracks to NULL album art.
    sqlx::query(r#"UPDATE "Track" SET "albumArtId"=NULL WHERE "albumArtId" IS NOT NULL"#)
        .execute(&mut **tx)
        .await?;

    // Although the albumArtId column is nullable in the database schema
    // Engine DJ chokes up when encountering NULL values. All tracks must
    // at least reference the NULL album art.

    // Clear AlbumArt.
    let purged_count = sqlx::query(r#"DELETE FROM "AlbumArt""#)
        .execute(&mut **tx)
        .await?
        .rows_affected();

    // Reset AUTOINCREMENT primary key sequence of AlbumArt.
    sqlx::query(r#"UPDATE "sqlite_sequence" SET "seq"=0 WHERE "name"='AlbumArt'"#)
        .execute(&mut **tx)
        .await?;

    // Insert NULL album art.
    let insert_result =
        sqlx::query(r#"INSERT INTO "AlbumArt" ("hash","albumArt") VALUES (NULL,NULL)"#)
            .execute(&mut **tx)
            .await?;
    let inserted_count = insert_result.rows_affected();
    debug_assert_eq!(inserted_count, 1);
    let album_art_id = insert_result.last_insert_rowid();
    debug_assert_eq!(album_art_id, 1);

    // Reset album art of all tracks to NULL album art.
    sqlx::query(r#"UPDATE "Track" SET "albumArtId"=?1"#)
        .execute(&mut **tx)
        .await?;

    // Do not count the NULL album art that has both been deleted and then re-inserted.
    debug_assert!(purged_count >= inserted_count);
    Ok(purged_count.saturating_sub(inserted_count))
}
