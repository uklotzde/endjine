// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use futures_util::stream::BoxStream;
use relative_path::RelativePath;
use sqlx::{FromRow, SqliteExecutor};

use crate::{AlbumArtId, DbUuid, UnixTimestamp};

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
    pub length: Option<u64>,
    pub bpm: Option<i64>,
    pub year: Option<i64>,
    pub path: Option<String>,
    pub filename: Option<String>,
    pub bitrate: Option<i64>,
    pub bpm_analyzed: Option<f64>,
    pub album_art_id: AlbumArtId,
    pub file_bytes: Option<u64>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub comment: Option<String>,
    pub label: Option<String>,
    pub composer: Option<String>,
    pub remixer: Option<String>,
    pub key: Option<u8>,
    pub rating: Option<i64>,
    pub album_art: Option<String>,
    pub time_last_played: Option<UnixTimestamp>,
    pub is_played: bool,
    pub file_type: Option<String>,
    pub is_analyzed: bool,
    pub date_created: UnixTimestamp,
    pub date_added: UnixTimestamp,
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
    pub origin_track_id: Option<TrackId>,
    pub streaming_flags: i64,
    pub explicit_lyrics: bool,
    pub last_edit_time: UnixTimestamp,
}

/// References a track within the local and its origin database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct TrackRef {
    pub id: TrackId,
    pub origin_database_uuid: Option<DbUuid>,
    pub origin_track_id: Option<TrackId>,
}

impl Track {
    /// Default non-null album art.
    ///
    /// Engine DJ writes this string into the `albumArt` column. But many
    /// tracks just contain NULL. This value doesn't seem to be needed and
    /// the column value could safely be set to NULL.
    pub const DEFAULT_ALBUM_ART: &str = "image://planck/0";

    /// Determines the base path for all relative track paths in the database.
    #[must_use]
    pub fn base_path(database_path: &Path) -> Option<&Path> {
        grandparent_path(database_path)
    }

    /// Determines the file path given the base path.
    ///
    /// The resulting path is not canonicalized.
    #[must_use]
    pub fn file_path(&self, base_path: &Path) -> Option<PathBuf> {
        self.path
            .as_ref()
            .map(|path| RelativePath::new(path).to_path(base_path))
    }

    /// Fetches all [`Track`]s asynchronously.
    ///
    /// Unfiltered and in no particular order.
    #[must_use]
    pub fn fetch_all<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(r#"SELECT * FROM "Track" ORDER BY "id""#).fetch(executor)
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

    /// Finds the [`TrackRef`] for the given path.
    ///
    /// The path must be relative and match the path in the database.
    pub async fn find_ref_by_path(
        executor: impl SqliteExecutor<'_>,
        path: &Path,
    ) -> sqlx::Result<Option<TrackRef>> {
        debug_assert!(path.is_relative());
        debug_assert!(path.starts_with(RELATIVE_TRACK_PATH_PREFIX));
        sqlx::query_as(
            r#"SELECT "id","originDatabaseUuid","originTrackId" FROM "Track" WHERE "path"=?1"#,
        )
        .bind(path.display().to_string())
        .fetch_optional(executor)
        .await
    }
}

pub const RELATIVE_TRACK_PATH_PREFIX: &str = "..";

pub fn resolve_track_path<'p>(
    base_path: &Path,
    track_path: &'p Path,
) -> anyhow::Result<Cow<'p, Path>> {
    debug_assert!(base_path.is_absolute());
    if track_path.is_relative() {
        let resolved_path = if track_path.starts_with(RELATIVE_TRACK_PATH_PREFIX) {
            // Leave relative paths as is.
            Cow::Borrowed(track_path)
        } else {
            Cow::Owned(Path::new(RELATIVE_TRACK_PATH_PREFIX).join(track_path))
        };
        return Ok(resolved_path);
    }
    debug_assert!(track_path.is_absolute());
    let relative_path = track_path.strip_prefix(base_path)?;
    let resolved_path = Path::new(RELATIVE_TRACK_PATH_PREFIX).join(relative_path);
    Ok(Cow::Owned(resolved_path))
}

#[must_use]
fn grandparent_path(path: &Path) -> Option<&Path> {
    path.parent().and_then(Path::parent)
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(not(target_os = "windows"))]
    fn resolve_track_path() {
        use std::path::Path;

        use crate::track::RELATIVE_TRACK_PATH_PREFIX;

        let base_path = Path::new("/foo/bar").to_path_buf();

        // Resolvable paths.
        assert_eq!(
            super::resolve_track_path(&base_path, Path::new(RELATIVE_TRACK_PATH_PREFIX)).unwrap(),
            Path::new(RELATIVE_TRACK_PATH_PREFIX)
        );
        assert_eq!(
            super::resolve_track_path(&base_path, &base_path).unwrap(),
            Path::new(RELATIVE_TRACK_PATH_PREFIX)
        );
        assert_eq!(
            super::resolve_track_path(&base_path, &base_path.join("lorem")).unwrap(),
            Path::new(RELATIVE_TRACK_PATH_PREFIX).join("lorem")
        );
        assert_eq!(
            super::resolve_track_path(&base_path, &base_path.join("lorem").join("ipsum")).unwrap(),
            Path::new(RELATIVE_TRACK_PATH_PREFIX)
                .join("lorem")
                .join("ipsum")
        );
        assert_eq!(
            super::resolve_track_path(
                &base_path,
                &base_path.join(RELATIVE_TRACK_PATH_PREFIX).join("bar")
            )
            .unwrap(),
            Path::new(RELATIVE_TRACK_PATH_PREFIX)
                .join(RELATIVE_TRACK_PATH_PREFIX)
                .join("bar")
        );

        // Unresolvable paths.
        assert!(super::resolve_track_path(&base_path, &Path::new("/").join("foo")).is_err());
        assert!(super::resolve_track_path(&base_path, &Path::new("/").join("bar")).is_err());
        assert!(
            super::resolve_track_path(&base_path, &Path::new("/").join("bar").join("foo")).is_err()
        );
    }
}
