// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::borrow::Cow;

use anyhow::bail;
use futures_util::stream::BoxStream;
use relative_path::RelativePath;
use sqlx::{FromRow, SqliteExecutor};

use crate::{AlbumArtId, DbUuid, FilePath, LibraryPath, UnixTimestamp};

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
    // The columns is nullable, but a database trigger ensures that could never become NULL.
    pub origin_database_uuid: DbUuid,
    // The columns is nullable, but a database trigger ensures that could never become NULL.
    pub origin_track_id: TrackId,
    pub streaming_flags: i64,
    pub explicit_lyrics: bool,
    pub last_edit_time: UnixTimestamp,
}

impl Track {
    #[must_use]
    pub const fn to_ref(&self) -> TrackRef {
        let Self {
            id,
            origin_track_id,
            origin_database_uuid,
            ..
        } = self;
        TrackRef {
            id: *id,
            origin_track_id: *origin_track_id,
            origin_database_uuid: *origin_database_uuid,
        }
    }
}

/// References a track within the local and its origin database.
///
/// Subset of fields/columns from [`Track`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct TrackRef {
    id: TrackId,
    origin_track_id: TrackId,
    origin_database_uuid: DbUuid,
}

impl TrackRef {
    pub fn to_origin(&self, local_db_uuid: DbUuid) -> anyhow::Result<OriginTrackRef> {
        let Self {
            id,
            origin_track_id,
            origin_database_uuid,
        } = self;
        if (local_db_uuid == *origin_database_uuid) && id != origin_track_id {
            bail!("mismatching track ids");
        }
        Ok(OriginTrackRef {
            id: *origin_track_id,
            db_uuid: *origin_database_uuid,
        })
    }
}

/// References a track within its origin database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OriginTrackRef {
    pub id: TrackId,
    pub db_uuid: DbUuid,
}

impl Track {
    /// Default non-null album art.
    ///
    /// Engine DJ writes this string into the `albumArt` column. But many
    /// tracks just contain NULL. This value doesn't seem to be needed and
    /// the column value could safely be set to NULL.
    pub const DEFAULT_ALBUM_ART: &str = "image://planck/0";

    /// Determines the file path given the library path.
    #[must_use]
    pub fn to_file_path(&self, library_path: &LibraryPath) -> Option<FilePath<'static>> {
        self.path.as_ref().map(|path| {
            let mut file_path = library_path.file_path().clone();
            file_path.append_relative_suffix(path);
            file_path
        })
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

/// Parent directory of "Engine Library".
const RELATIVE_TRACK_PATH_PREFIX: &str = "..";

pub fn import_track_file_path<'p>(
    library_path: &LibraryPath,
    mut file_path: FilePath<'p>,
) -> anyhow::Result<Cow<'p, RelativePath>> {
    if file_path.is_relative() {
        if file_path.relative().starts_with(RELATIVE_TRACK_PATH_PREFIX) {
            // Leave relative with matching prefix as is.
            return Ok(file_path.into_relative());
        }
        file_path.prepend_relative_prefix(RelativePath::new(RELATIVE_TRACK_PATH_PREFIX));
        return Ok(file_path.into_relative());
    }
    let Some(base_path) = library_path.relative().parent() else {
        bail!("invalid library path");
    };
    if !file_path.strip_relative_prefix(base_path) {
        bail!("mismatching base path \"{base_path}\"");
    }
    file_path.prepend_relative_prefix(RelativePath::new(RELATIVE_TRACK_PATH_PREFIX));
    Ok(file_path.into_relative())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use relative_path::RelativePath;

    use crate::{FilePath, LIBRARY_DIRECTORY_NAME, LibraryPath};

    use super::RELATIVE_TRACK_PATH_PREFIX;

    #[test]
    fn import_track_file_path() {
        #[cfg(target_os = "windows")]
        let root_path = Path::new("C:\\");
        #[cfg(not(target_os = "windows"))]
        let root_path = Path::new("/");
        assert!(root_path.is_absolute());

        let base_path = RelativePath::new("foo/bar").to_relative_path_buf();
        let abs_base_path = base_path.to_path(root_path);

        let db_file_path = FilePath::import_path(
            &abs_base_path
                .join(LIBRARY_DIRECTORY_NAME)
                .join("Database2")
                .join("m.db"),
        );
        let library_path = LibraryPath::new(&db_file_path).unwrap();

        // Valid paths.
        assert_eq!(
            super::import_track_file_path(
                &library_path,
                FilePath::import_path(RELATIVE_TRACK_PATH_PREFIX)
            )
            .unwrap(),
            RelativePath::new(RELATIVE_TRACK_PATH_PREFIX)
        );
        assert_eq!(
            super::import_track_file_path(&library_path, FilePath::import_path(&abs_base_path))
                .unwrap(),
            RelativePath::new(RELATIVE_TRACK_PATH_PREFIX)
        );
        assert_eq!(
            super::import_track_file_path(
                &library_path,
                FilePath::import_path(&abs_base_path.join("lorem"))
            )
            .unwrap(),
            RelativePath::new(RELATIVE_TRACK_PATH_PREFIX).join("lorem")
        );
        assert_eq!(
            super::import_track_file_path(
                &library_path,
                FilePath::import_path(&abs_base_path.join("lorem").join("ipsum"))
            )
            .unwrap(),
            RelativePath::new(RELATIVE_TRACK_PATH_PREFIX)
                .join("lorem")
                .join("ipsum")
        );
        assert_eq!(
            super::import_track_file_path(
                &library_path,
                FilePath::import_path(&abs_base_path.join(RELATIVE_TRACK_PATH_PREFIX).join("bar"))
            )
            .unwrap(),
            RelativePath::new(RELATIVE_TRACK_PATH_PREFIX)
        );

        // Invalid paths.
        assert!(
            super::import_track_file_path(
                &library_path,
                FilePath::import_path(&RelativePath::new("foo").to_path(root_path))
            )
            .is_err()
        );
        assert!(
            super::import_track_file_path(
                &library_path,
                FilePath::import_path(&RelativePath::new("bar").to_path(root_path))
            )
            .is_err()
        );
        assert!(
            super::import_track_file_path(
                &library_path,
                FilePath::import_path(&RelativePath::new("bar/foo").to_path(root_path))
            )
            .is_err()
        );
    }
}
