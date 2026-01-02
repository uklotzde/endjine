// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use futures_util::stream::BoxStream;
use relative_path::RelativePath;
use sqlx::{FromRow, SqliteExecutor};

use crate::{AlbumArtId, DbUuid, UnixTimestamp, split_and_normalize_file_path};

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

    /// Splits the database file path into a root path and the relat the base path for all relative track paths in the database.
    ///
    /// Returns a tuple with both the `root_path` and the relative `base_path`. The `root_path`
    /// is empty if the database file path is relative.
    #[must_use]
    pub fn split_root_base_path(
        database_file_path: &Path,
    ) -> Option<(PathBuf, Cow<'_, RelativePath>)> {
        let (root_path, database_path) = split_and_normalize_file_path(database_file_path);
        let base_path = match database_path {
            Cow::Borrowed(database_path) => grandparent_path(database_path).map(Cow::Borrowed),
            Cow::Owned(database_path) => grandparent_path(&database_path)
                .map(RelativePath::to_relative_path_buf)
                .map(Cow::Owned),
        };
        base_path.map(|base_path| (root_path, base_path))
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
        path: &RelativePath,
    ) -> sqlx::Result<Option<TrackRef>> {
        debug_assert!(path.starts_with(RELATIVE_TRACK_PATH_PREFIX));
        sqlx::query_as(
            r#"SELECT "id","originDatabaseUuid","originTrackId" FROM "Track" WHERE "path"=?1"#,
        )
        .bind(path.to_string())
        .fetch_optional(executor)
        .await
    }
}

pub const RELATIVE_TRACK_PATH_PREFIX: &str = "..";

pub fn normalize_track_file_path<'p>(
    base_path: &RelativePath,
    track_file_path: &'p Path,
) -> anyhow::Result<(PathBuf, Cow<'p, RelativePath>)> {
    debug_assert!(base_path.is_normalized());
    let (root_path, normalized_track_path) = split_and_normalize_file_path(track_file_path);
    if track_file_path.is_relative() {
        if normalized_track_path.starts_with(RELATIVE_TRACK_PATH_PREFIX) {
            // Leave relative with matching prefix as is.
            return Ok((root_path, normalized_track_path));
        }
        return Ok((
            root_path,
            Cow::Owned(RelativePath::new(RELATIVE_TRACK_PATH_PREFIX).join(normalized_track_path)),
        ));
    }
    debug_assert!(track_file_path.is_absolute());
    let relative_path = normalized_track_path
        .strip_prefix(base_path)
        .map_err(|_| anyhow!("strip base path prefix"))?;
    Ok((
        root_path,
        Cow::Owned(RelativePath::new(RELATIVE_TRACK_PATH_PREFIX).join(relative_path)),
    ))
}

#[must_use]
fn grandparent_path(path: &RelativePath) -> Option<&RelativePath> {
    path.parent().and_then(RelativePath::parent)
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, path::PathBuf};

    #[test]
    fn normalize_track_file_path() {
        use std::path::Path;

        use relative_path::RelativePath;

        use crate::track::RELATIVE_TRACK_PATH_PREFIX;

        #[cfg(target_os = "windows")]
        let root_path = Path::new("C:\\");
        #[cfg(not(target_os = "windows"))]
        let root_path = Path::new("/");
        assert!(root_path.is_absolute());

        let base_path = RelativePath::new("foo/bar").to_relative_path_buf();
        let abs_base_path = base_path.to_path(root_path);

        // Resolvable paths.
        assert_eq!(
            super::normalize_track_file_path(&base_path, Path::new(RELATIVE_TRACK_PATH_PREFIX))
                .unwrap(),
            (
                PathBuf::new(),
                Cow::Borrowed(RelativePath::new(RELATIVE_TRACK_PATH_PREFIX))
            )
        );
        assert_eq!(
            super::normalize_track_file_path(&base_path, &abs_base_path).unwrap(),
            (
                root_path.to_path_buf(),
                Cow::Borrowed(RelativePath::new(RELATIVE_TRACK_PATH_PREFIX))
            )
        );
        assert_eq!(
            super::normalize_track_file_path(&base_path, &abs_base_path.join("lorem")).unwrap(),
            (
                root_path.to_path_buf(),
                Cow::Owned(RelativePath::new(RELATIVE_TRACK_PATH_PREFIX).join("lorem"))
            )
        );
        assert_eq!(
            super::normalize_track_file_path(
                &base_path,
                &abs_base_path.join("lorem").join("ipsum")
            )
            .unwrap(),
            (
                root_path.to_path_buf(),
                Cow::Owned(
                    RelativePath::new(RELATIVE_TRACK_PATH_PREFIX)
                        .join("lorem")
                        .join("ipsum")
                )
            )
        );
        assert_eq!(
            super::normalize_track_file_path(
                &base_path,
                &abs_base_path.join(RELATIVE_TRACK_PATH_PREFIX).join("bar")
            )
            .unwrap(),
            (
                root_path.to_path_buf(),
                Cow::Borrowed(RelativePath::new(RELATIVE_TRACK_PATH_PREFIX))
            )
        );

        // Unresolvable paths.
        assert!(
            super::normalize_track_file_path(
                &base_path,
                &RelativePath::new("foo").to_path(root_path)
            )
            .is_err()
        );
        assert!(
            super::normalize_track_file_path(
                &base_path,
                &RelativePath::new("bar").to_path(root_path)
            )
            .is_err()
        );
        assert!(
            super::normalize_track_file_path(
                &base_path,
                &RelativePath::new("bar/foo").to_path(root_path)
            )
            .is_err()
        );
    }
}
