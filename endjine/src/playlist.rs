// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::borrow::{Borrow, Cow};

use anyhow::{Context as _, bail};
use futures_util::{
    StreamExt as _, TryStreamExt as _,
    stream::{BoxStream, FuturesOrdered},
};
use itertools::Itertools;
use relative_path::RelativePath;
use sqlx::{
    FromRow, SqliteExecutor, SqlitePool, sqlite::SqliteQueryResult, types::time::PrimitiveDateTime,
};

use crate::{DbUuid, FilePath, Track, TrackId, TrackRef, import_track_file_path};

crate::db_id!(PlaylistId);

const MIN_MEMBERSHIP_REFERENCE: i64 = 1;

#[must_use]
fn next_membership_reference(membership_reference: i64) -> i64 {
    debug_assert!(membership_reference >= MIN_MEMBERSHIP_REFERENCE);
    membership_reference + 1
}

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Playlist {
    pub id: PlaylistId,
    pub title: String,
    pub parent_list_id: PlaylistId,
    pub is_persisted: bool,
    pub next_list_id: PlaylistId,
    /// UTC timestamp encoded as plain date/time.
    pub last_edit_time: PrimitiveDateTime,
    pub is_explicitly_exported: bool,
}

impl Playlist {
    /// Fetches all [`Playlist`]s.
    ///
    /// Unfiltered and in no particular order.
    #[must_use]
    pub fn fetch_all<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(r#"SELECT * FROM "Playlist" ORDER BY "id""#).fetch(executor)
    }

    /// Fetches all empty [`Playlist`]s without children.
    ///
    /// In no particular order.
    #[must_use]
    pub fn fetch_all_empty_without_children<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(
            r#"SELECT * FROM "Playlist"
            WHERE "id" NOT IN (SELECT "listId" FROM "PlaylistEntity")
            AND "id" NOT IN (SELECT "parentListId" FROM "Playlist")
            ORDER BY "id""#,
        )
        .fetch(executor)
    }

    /// Deletes a playlist from the database.
    pub async fn delete(&self, executor: impl SqliteExecutor<'_>) -> sqlx::Result<bool> {
        sqlx::query(r#"DELETE FROM "Playlist" WHERE "id"=?1"#)
            .bind(self.id)
            .execute(executor)
            .await
            .map(|result| {
                debug_assert!(result.rows_affected() <= 1);
                result.rows_affected() > 0
            })
    }

    /// Deletes all empty [`Playlist`]s without children.
    pub async fn delete_all_empty_without_children(
        executor: impl SqliteExecutor<'_>,
    ) -> sqlx::Result<u64> {
        sqlx::query(
            r#"DELETE FROM "Playlist"
                WHERE "id" NOT IN (SELECT "listId" FROM "PlaylistEntity")
                AND "id" NOT IN (SELECT "parentListId" FROM "Playlist")"#,
        )
        .execute(executor)
        .await
        .map(|result| result.rows_affected())
    }

    /// Loads a single [`Playlist`] by ID.
    ///
    /// Returns `Ok(None)` if the requested [`Playlist`] has not been found.
    pub async fn try_load(
        executor: impl SqliteExecutor<'_>,
        id: PlaylistId,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as(r#"SELECT * FROM "Playlist" WHERE "id"=?1"#)
            .bind(id)
            .fetch_optional(executor)
            .await
    }

    pub async fn find_id_by_path(
        executor: impl SqliteExecutor<'_>,
        path: &str,
    ) -> sqlx::Result<Option<PlaylistId>> {
        let path = if path.ends_with(PLAYLIST_PATH_SEGMENT_SEPARATOR) {
            Cow::Borrowed(path)
        } else {
            // Terminate the path.
            Cow::Owned([path, PLAYLIST_PATH_SEGMENT_SEPARATOR].concat())
        };
        sqlx::query_scalar(r#"SELECT "id" FROM "PlaylistPath" WHERE "path"=?1"#)
            .bind(path)
            .fetch_optional(executor)
            .await
    }

    /// Appends tracks to a playlist.
    ///
    /// Returns all duplicate tracks that have been ignored.
    ///
    /// Must run within a transaction in isolation.
    pub async fn append_tracks<'e, E>(
        mut executor: impl FnMut() -> E,
        id: PlaylistId,
        track_refs: impl IntoIterator<Item = PlaylistTrackRef>,
    ) -> anyhow::Result<Vec<PlaylistTrackRef>>
    where
        E: SqliteExecutor<'e>,
    {
        let last_entity = PlaylistEntity::try_load_last_of_list(&mut executor, id).await?;

        let (mut prev_entity_id, mut next_membership_ref) = last_entity.map_or(
            (PlaylistEntityId::INVALID_ZERO, MIN_MEMBERSHIP_REFERENCE),
            |PlaylistEntity {
                 id,
                 membership_reference,
                 ..
             }| (id, next_membership_reference(membership_reference)),
        );

        // Append each track as a new playlist entry.
        let mut ignored_track_refs = Vec::new();
        for track_ref in track_refs {
            let PlaylistTrackRef {
                track_id,
                database_uuid,
            } = &track_ref;
            let query_result = sqlx::query(
                r#"INSERT OR IGNORE INTO "PlaylistEntity"
                   ("listId", "trackId", "databaseUuid", "nextEntityId", "membershipReference")
                   VALUES (?1, ?2, ?3, ?4, ?5)"#,
            )
            .bind(id)
            .bind(track_id)
            .bind(database_uuid)
            .bind(PlaylistEntityId::INVALID_ZERO)
            .bind(next_membership_ref)
            .execute(executor())
            .await?;

            debug_assert!(query_result.rows_affected() <= 1);
            if query_result.rows_affected() == 0 {
                // Ignore duplicate tracks.
                ignored_track_refs.push(track_ref);
                continue;
            }

            let new_entity_id = PlaylistEntityId::new(query_result.last_insert_rowid());

            // Update the previous entry to point to this new entry.
            if prev_entity_id.is_valid() {
                // Maintain the linked list structure.
                sqlx::query(r#"UPDATE "PlaylistEntity" SET "nextEntityId"=?1 WHERE "id"=?2"#)
                    .bind(new_entity_id)
                    .bind(prev_entity_id)
                    .execute(executor())
                    .await?;
            }

            // Set up for next iteration.
            prev_entity_id = new_entity_id;
            next_membership_ref = next_membership_reference(next_membership_ref);
        }

        Ok(ignored_track_refs)
    }
}

pub async fn resolve_playlist_track_refs_from_file_paths<'p>(
    pool: &SqlitePool,
    local_database_uuid: DbUuid,
    base_path: &RelativePath,
    track_paths: impl IntoIterator<Item = FilePath<'p>>,
) -> anyhow::Result<Vec<PlaylistTrackRef>> {
    let track_refs_fut = track_paths
        .into_iter()
        .map(|track_path| {
            import_track_file_path(base_path, track_path)
                .map(|track_path| async move {
                    let track_ref = Track::find_ref_by_path(pool, &track_path)
                        .await
                        .with_context(|| {
                            format!("find reference of track path \"{track_path}\"")
                        })?;
                    let Some(track_ref) = track_ref else {
                        bail!("unknown track path \"{track_path}\"");
                    };
                    PlaylistTrackRef::new(track_ref, local_database_uuid)
                })
                .context("import track file path \"{track_path}\"")
        })
        .collect::<anyhow::Result<FuturesOrdered<_>>>()?;
    track_refs_fut.try_collect::<Vec<_>>().await
}

crate::db_id!(PlaylistEntityId);

/// Entry in a [`Playlist`].
///
/// The terminology used in the schema is confusing and the table
/// should have been named `PlaylistEntry` instead of `PlaylistEntity`.
#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PlaylistEntity {
    pub id: PlaylistEntityId,
    pub list_id: PlaylistId,
    pub track_id: TrackId,
    pub database_uuid: DbUuid,
    pub next_entity_id: PlaylistEntityId,
    pub membership_reference: i64,
}

impl PlaylistEntity {
    #[must_use]
    pub const fn track_ref(&self) -> PlaylistTrackRef {
        let Self {
            track_id,
            database_uuid,
            ..
        } = self;
        PlaylistTrackRef {
            track_id: *track_id,
            database_uuid: *database_uuid,
        }
    }

    /// Fetches all [`PlaylistEntity`]s.
    ///
    /// Unfiltered and in no particular order.
    #[must_use]
    pub fn fetch_all<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(r#"SELECT * FROM "PlaylistEntity" ORDER BY "id""#).fetch(executor)
    }

    /// Fetches all entries of a [`Playlist`].
    ///
    /// Ordered by the canonical position in the playlist.
    #[must_use]
    pub fn fetch_list<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
        list_id: PlaylistId,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(
            r#"SELECT * FROM "PlaylistEntity" WHERE "listId"=?1 ORDER BY "membershipReference""#,
        )
        .bind(list_id)
        .fetch(executor)
    }

    /// Loads all entries of a [`Playlist`].
    ///
    /// Ordered by the canonical position in the playlist.
    pub async fn load_list(
        executor: impl SqliteExecutor<'_>,
        list_id: PlaylistId,
    ) -> sqlx::Result<Vec<Self>> {
        sqlx::query_as(
            r#"SELECT * FROM "PlaylistEntity" WHERE "listId"=?1 ORDER BY "membershipReference""#,
        )
        .bind(list_id)
        .fetch_all(executor)
        .await
    }

    /// Deletes all entries of a [`Playlist`].
    pub async fn delete_list(
        executor: impl SqliteExecutor<'_>,
        list_id: PlaylistId,
    ) -> sqlx::Result<SqliteQueryResult> {
        sqlx::query(r#"DELETE FROM "PlaylistEntity" WHERE "listId"=?1"#)
            .bind(list_id)
            .execute(executor)
            .await
    }

    pub async fn count_list(
        executor: impl SqliteExecutor<'_>,
        list_id: PlaylistId,
    ) -> sqlx::Result<u64> {
        let count: i64 =
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "PlaylistEntity" WHERE "listId"=?1"#)
                .bind(list_id)
                .fetch_one(executor)
                .await?;
        debug_assert!(count >= 0);
        Ok(count.cast_unsigned())
    }

    /// Reads the (unambiguous) database UUID for this playlist's entries.
    ///
    /// Returns `Ok(None)` if the requested [`Playlist`] has no entries.
    pub async fn try_load_db_uuid_of_list<'e, E>(
        mut executor: impl FnMut() -> E,
        list_id: PlaylistId,
    ) -> anyhow::Result<Option<DbUuid>>
    where
        E: SqliteExecutor<'e>,
    {
        let mut uuid_results = sqlx::query_scalar(
            r#"SELECT DISTINCT "databaseUuid" FROM "PlaylistEntity" WHERE "listId"=?1 LIMIT 2"#,
        )
        .bind(list_id)
        .fetch(executor());

        let Some(uuid_result) = uuid_results.next().await else {
            // Playlist is empty.
            debug_assert_eq!(
                PlaylistEntity::count_list(executor(), list_id).await.ok(),
                Some(0)
            );
            return Ok(None);
        };
        let uuid = uuid_result?;
        if uuid_results.next().await.is_some() {
            bail!("playlist entries reference multiple database UUIDs");
        }
        Ok(Some(uuid))
    }

    /// Loads the last item of a list.
    ///
    /// Returns `Ok(None)` if the list is empty. Fails if the last item
    /// is ambiguous.
    pub async fn try_load_last_of_list<'e, E>(
        mut executor: impl FnMut() -> E,
        list_id: PlaylistId,
    ) -> anyhow::Result<Option<Self>>
    where
        E: SqliteExecutor<'e>,
    {
        let mut last_entity_results = sqlx::query_as(
            r#"SELECT * FROM "PlaylistEntity"
               WHERE "listId"=?1
               ORDER BY "membershipReference" DESC
               LIMIT 2"#,
        )
        .bind(list_id)
        .fetch(executor());

        let Some(last_entity_result) = last_entity_results.next().await else {
            // Playlist is empty.
            debug_assert_eq!(
                PlaylistEntity::count_list(executor(), list_id).await.ok(),
                Some(0)
            );
            return Ok(None);
        };
        let last_entity: PlaylistEntity = last_entity_result?;
        debug_assert!(last_entity.membership_reference >= MIN_MEMBERSHIP_REFERENCE);
        if last_entity.next_entity_id.is_valid() {
            bail!(
                "last entry (id = {last_id}) in playlist has next entry (id = {next_id})",
                last_id = last_entity.id,
                next_id = last_entity.next_entity_id
            );
        }
        if let Some(next_to_last_entity_result) = last_entity_results.next().await {
            let next_to_last_entity = next_to_last_entity_result?;
            debug_assert!(
                next_to_last_entity.membership_reference <= last_entity.membership_reference
            );
            if next_to_last_entity.membership_reference == last_entity.membership_reference {
                bail!("playlist with multiple last entries");
            }
        }
        Ok(Some(last_entity))
    }

    /// Loads a single [`PlaylistEntity`] by ID.
    ///
    /// Returns `Ok(None)` if the requested [`PlaylistEntity`] has not been found.
    pub async fn try_load(
        executor: impl SqliteExecutor<'_>,
        id: PlaylistEntityId,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as(r#"SELECT * FROM "PlaylistEntity" WHERE "id"=?1"#)
            .bind(id)
            .fetch_optional(executor)
            .await
    }
}

/// References a track with(-in) its origin database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PlaylistTrackRef {
    pub track_id: TrackId,
    pub database_uuid: DbUuid,
}

impl PlaylistTrackRef {
    pub fn new(track_ref: TrackRef, local_database_uuid: DbUuid) -> anyhow::Result<Self> {
        match track_ref {
            TrackRef {
                id,
                origin_database_uuid: Some(origin_database_uuid),
                origin_track_id: Some(origin_track_id),
            } => {
                if (origin_database_uuid == local_database_uuid) && id != origin_track_id {
                    bail!("mismatching track ids");
                }
                Ok(Self {
                    track_id: origin_track_id,
                    database_uuid: origin_database_uuid,
                })
            }
            TrackRef {
                id,
                origin_database_uuid: None,
                origin_track_id: None,
            } => Ok(Self {
                track_id: id,
                database_uuid: local_database_uuid,
            }),
            _ => bail!("invalid track reference {track_ref:?}"),
        }
    }
}

crate::db_id!(PlaylistAllChildrenId);

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PlaylistAllChildren {
    pub id: PlaylistAllChildrenId,
    pub child_list_id: PlaylistId,
}

crate::db_id!(PlaylistAllParentId);

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PlaylistAllParent {
    pub id: PlaylistAllParentId,
    pub parent_list_id: PlaylistId,
}

crate::db_id!(PlaylistPathId);

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PlaylistPath {
    pub id: i64,
    pub path: String,
    pub position: i64,
}

pub const PLAYLIST_PATH_SEGMENT_SEPARATOR: &str = ";";

#[must_use]
pub fn is_valid_playlist_path_segment(segment: &str) -> bool {
    !segment.is_empty() && !segment.contains(PLAYLIST_PATH_SEGMENT_SEPARATOR)
}

#[must_use]
pub fn concat_playlist_path_segments_to_string<'s, S>(
    segments: impl IntoIterator<Item = &'s S>,
) -> String
where
    S: Borrow<str> + ?Sized + 's,
{
    #[expect(unstable_name_collisions, reason = "itertools")]
    segments
        .into_iter()
        .map(Borrow::borrow)
        .inspect(|segment| {
            debug_assert!(is_valid_playlist_path_segment(segment));
        })
        .intersperse(PLAYLIST_PATH_SEGMENT_SEPARATOR)
        .chain(std::iter::once(PLAYLIST_PATH_SEGMENT_SEPARATOR))
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn concat_playlist_path_segments_to_string() {
        assert_eq!(
            super::concat_playlist_path_segments_to_string({
                let empty_array: [&str; 0] = [];
                empty_array
            }),
            ";"
        );
        assert_eq!(
            super::concat_playlist_path_segments_to_string(["foo"]),
            "foo;"
        );
        assert_eq!(
            super::concat_playlist_path_segments_to_string(["foo", "bar"]),
            "foo;bar;"
        );
        assert_eq!(
            super::concat_playlist_path_segments_to_string(["foo bar"]),
            "foo bar;"
        );
    }
}
