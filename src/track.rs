// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use futures_util::stream::BoxStream;
use sqlx::{FromRow, SqliteExecutor, types::time::OffsetDateTime};

use crate::{AlbumArtId, DbUuid};

crate::db_id!(TrackId);

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Reverse-engineered from database schema."
)]
pub struct Track {
    pub id: TrackId,
    pub play_order: Option<i64>,
    pub length: Option<i64>,
    pub bpm: Option<i64>,
    pub year: Option<i64>,
    pub path: Option<String>,
    pub filename: Option<String>,
    pub bitrate: Option<i64>,
    pub bpm_analyzed: Option<f64>,
    pub album_art_id: AlbumArtId,
    pub file_bytes: Option<i64>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub comment: Option<String>,
    pub label: Option<String>,
    pub composer: Option<String>,
    pub remixer: Option<String>,
    pub key: Option<i64>,
    pub rating: Option<i64>,
    pub album_art: Option<String>,
    pub time_last_played: Option<OffsetDateTime>,
    pub is_played: bool,
    pub file_type: Option<String>,
    pub is_analyzed: bool,
    pub date_created: Option<OffsetDateTime>,
    pub date_added: Option<OffsetDateTime>,
    pub is_available: bool,
    pub is_metadata_of_packed_track_changed: bool,
    // Typo in column name of database schema requires renaming.
    #[sqlx(rename = "isPerfomanceDataOfPackedTrackChanged")]
    pub is_performance_data_of_packed_track_changed: bool,
    pub played_indicator: Option<i64>,
    pub is_metadata_imported: bool,
    pub pdb_import_key: Option<i64>,
    pub streaming_source: Option<String>,
    pub uri: Option<String>,
    pub is_beat_grid_locked: bool,
    pub origin_database_uuid: Option<DbUuid>,
    pub origin_track_id: Option<i64>,
    pub streaming_flags: i64,
    pub explicit_lyrics: bool,
    pub last_edit_time: OffsetDateTime,
}

impl Track {
    // Engine DJ writes this string into the `albumArt` column. But many
    // tracks just contain NULL. This value doesn't seem to be needed and
    // the column value could safely be set to NULL.
    pub const DEFAULT_ALBUM_ART: &str = "image://planck/0";

    /// Fetches all [`Track`]s asynchronously.
    ///
    /// Unfiltered and in no particular order.
    #[must_use]
    pub fn fetch_all<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(r#"SELECT * FROM "Track""#).fetch(executor)
    }

    /// Loads a single [`Track`] by ID.
    ///
    /// Returns `Ok(None)` if the requested [`Track`] has not been found.
    pub async fn try_load(
        executor: impl SqliteExecutor<'_>,
        id: TrackId,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as(r#"SELECT * FROM "Track" WHERE "id"=?1"#)
            .bind(id)
            .fetch_optional(executor)
            .await
    }

    /// Reset unused default album art for tracks with album art.
    pub async fn reset_unused_default_album_art(
        executor: impl SqliteExecutor<'_>,
    ) -> sqlx::Result<u64> {
        let result = sqlx::query(r#"UPDATE "Track" SET "albumArt"=NULL WHERE "albumArt"=?1 AND "albumArtId" IS NOT NULL"#)
            .bind(Self::DEFAULT_ALBUM_ART)
            .execute(executor)
            .await?;
        Ok(result.rows_affected())
    }
}
