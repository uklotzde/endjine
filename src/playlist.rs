// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::{
    borrow::{Borrow, Cow},
    path::Path,
};

use anyhow::bail;
use futures_util::{StreamExt as _, stream::BoxStream};
use itertools::Itertools;
use sqlx::{FromRow, SqliteExecutor, sqlite::SqliteQueryResult, types::time::PrimitiveDateTime};

use crate::{DbUuid, Track, TrackId, resolve_track_path};

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
    /// Fetches all [`Playlist`]s asynchronously.
    ///
    /// Unfiltered and in no particular order.
    #[must_use]
    pub fn fetch_all<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(r#"SELECT * FROM "Playlist" ORDER BY "id""#).fetch(executor)
    }

    /// Fetches all empty [`Playlist`]s without children asynchronously.
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

    /// Adds tracks to a playlist.
    ///
    /// This method appends tracks to the end of the playlist.
    ///
    /// Must run within a transaction in isolation.
    pub async fn add_tracks<'e, E>(
        mut executor: impl FnMut() -> E,
        id: PlaylistId,
        db_uuid: DbUuid,
        track_ids: impl IntoIterator<Item = TrackId>,
    ) -> sqlx::Result<()>
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
        for track_id in track_ids {
            let result = sqlx::query(
                r#"INSERT INTO "PlaylistEntity"
                   ("listId", "trackId", "databaseUuid", "nextEntityId", "membershipReference")
                   VALUES (?1, ?2, ?3, ?4, ?5)"#,
            )
            .bind(id)
            .bind(track_id)
            .bind(db_uuid)
            .bind(PlaylistEntityId::INVALID_ZERO)
            .bind(next_membership_ref)
            .execute(executor())
            .await?;

            let new_entity_id = PlaylistEntityId::new(result.last_insert_rowid());

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

        Ok(())
    }

    /// Adds tracks by path to a playlist.
    ///
    /// See also: [`add_tracks()`](Self::add_tracks).
    pub fn add_tracks_by_path<'e, 'p, E>(
        mut executor: impl FnMut() -> E + 'e,
        id: PlaylistId,
        db_uuid: DbUuid,
        base_path: &Path,
        track_paths: impl IntoIterator<Item = &'p Path>,
    ) -> anyhow::Result<impl Future<Output = anyhow::Result<()>> + 'e>
    where
        E: SqliteExecutor<'e>,
    {
        let track_paths = track_paths
            .into_iter()
            .map(|track_path| resolve_track_path(base_path, track_path).map(Cow::into_owned))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(async move {
            let mut track_ids = Vec::with_capacity(track_paths.len());
            for track_path in track_paths {
                let Some(track_id) = Track::find_id_by_path(executor(), &track_path).await? else {
                    bail!(
                        "unknown track path \"{track_path}\"",
                        track_path = track_path.display()
                    );
                };
                track_ids.push(track_id);
            }
            Self::add_tracks(executor, id, db_uuid, track_ids)
                .await
                .map_err(Into::into)
        })
    }

    /// Replaces all tracks in a playlist.
    ///
    /// This method replaces all existing tracks in the playlist with the provided track IDs.
    /// It reuses existing entries where possible.
    ///
    /// Must run within a transaction in isolation.
    pub async fn replace_tracks<'e, E>(
        mut executor: impl FnMut() -> E,
        id: PlaylistId,
        db_uuid: DbUuid,
        track_ids: impl IntoIterator<Item = TrackId>,
    ) -> sqlx::Result<()>
    where
        E: SqliteExecutor<'e>,
    {
        let mut existing_entries = PlaylistEntity::load_list(executor(), id).await?.into_iter();
        let mut track_ids = track_ids.into_iter();
        let mut last_id_membership_reference = None;
        while let Some(next_track_id) = track_ids.next() {
            let Some(next_entry) = existing_entries.next() else {
                // All existing entries have been reused.
                // The remaining tracks need to be added as new entries.
                return Self::add_tracks(
                    executor,
                    id,
                    db_uuid,
                    std::iter::once(next_track_id).chain(track_ids),
                )
                .await;
            };

            // Update entry.
            if next_track_id != next_entry.track_id || db_uuid != next_entry.database_uuid {
                sqlx::query(
                    r#"UPDATE "PlaylistEntity"
                    SET "trackId"=?1, "databaseUuid"=?2
                    WHERE "id"=?3"#,
                )
                .bind(next_track_id)
                .bind(db_uuid)
                .bind(next_entry.id)
                .execute(executor())
                .await?;
            }

            // Prepare next iteration.
            let next_membership_reference = next_entry.membership_reference;
            debug_assert!(next_membership_reference >= MIN_MEMBERSHIP_REFERENCE);
            debug_assert!(
                last_id_membership_reference
                    .is_none_or(|(_, last_membership_reference)| last_membership_reference
                        < next_membership_reference)
            );
            last_id_membership_reference = Some((next_entry.id, next_membership_reference));
        }

        let Some((last_id, last_membership_reference)) = last_id_membership_reference else {
            // Playlist is empty.
            debug_assert_eq!(
                PlaylistEntity::count_list(executor(), id).await.ok(),
                Some(0)
            );
            let _query_result = PlaylistEntity::delete_list(executor(), id).await?;
            return Ok(());
        };

        // Terminate linked list.
        sqlx::query(
            r#"UPDATE "PlaylistEntity"
                   SET "nextEntityId"=?1
                   WHERE "id"=?2"#,
        )
        .bind(PlaylistEntityId::INVALID_ZERO)
        .bind(last_id)
        .execute(executor())
        .await?;

        // Delete unused/obsolete entries.
        let _query_result = sqlx::query(
            r#"DELETE FROM "PlaylistEntity" WHERE "listId"=?1 AND "membershipReference">?2"#,
        )
        .bind(id)
        .bind(last_membership_reference)
        .execute(executor())
        .await?;

        Ok(())
    }
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
    /// Fetches all [`PlaylistEntity`]s asynchronously.
    ///
    /// Unfiltered and in no particular order.
    #[must_use]
    pub fn fetch_all<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(r#"SELECT * FROM "PlaylistEntity" ORDER BY "id""#).fetch(executor)
    }

    /// Fetches all entries of a [`Playlist`] asynchronously.
    ///
    /// Ordered by the canonical position in the playlist.
    #[must_use]
    pub fn fetch_list<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
        list_id: PlaylistId,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(
            r#"SELECT * FROM "PlaylistEntity"
                WHERE "listId"=?1"
                ORDERED BY "membershipReference""#,
        )
        .bind(list_id)
        .fetch(executor)
    }

    /// Loads all entries of a [`Playlist`] asynchronously.
    ///
    /// Ordered by the canonical position in the playlist.
    pub async fn load_list(
        executor: impl SqliteExecutor<'_>,
        list_id: PlaylistId,
    ) -> sqlx::Result<Vec<Self>> {
        sqlx::query_as(
            r#"SELECT * FROM "PlaylistEntity"
                WHERE "listId"=?1"
                ORDERED BY "membershipReference""#,
        )
        .bind(list_id)
        .fetch_all(executor)
        .await
    }

    /// Deletes all entries of a [`Playlist`] asynchronously.
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
    ) -> sqlx::Result<Option<DbUuid>>
    where
        E: SqliteExecutor<'e>,
    {
        let mut fetch = sqlx::query_scalar(
            r#"SELECT DISTINCT "databaseUuid" FROM "PlaylistEntity" WHERE "listId"=?1 LIMIT 2"#,
        )
        .bind(list_id)
        .fetch(executor());

        let Some(uuid_result) = fetch.next().await else {
            // Playlist is empty.
            debug_assert_eq!(
                PlaylistEntity::count_list(executor(), list_id).await.ok(),
                Some(0)
            );
            return Ok(None);
        };
        let uuid = uuid_result?;
        if fetch.next().await.is_some() {
            return Err(sqlx::Error::Protocol(
                "playlist entries reference multiple database UUIDs".into(),
            ));
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
    ) -> sqlx::Result<Option<Self>>
    where
        E: SqliteExecutor<'e>,
    {
        let mut fetch = sqlx::query_as(
            r#"SELECT * FROM "PlaylistEntity"
               WHERE "listId"=?1 AND "membershipReference"=MAX("membershipReference")
               DESC LIMIT 2"#,
        )
        .bind(list_id)
        .fetch(executor());

        let Some(last_entity_result) = fetch.next().await else {
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
            return Err(sqlx::Error::Protocol(
                "last entry in playlist has next entry".into(),
            ));
        }
        if fetch.next().await.is_some() {
            return Err(sqlx::Error::Protocol(
                "playlist with multiple last entries".into(),
            ));
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
