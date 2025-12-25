// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::borrow::Borrow;

use futures_util::stream::BoxStream;
use itertools::Itertools;
use sqlx::{FromRow, SqliteExecutor, types::time::PrimitiveDateTime};

use crate::{DbUuid, TrackId};

crate::db_id!(PlaylistId);

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
        sqlx::query_as(r#"SELECT * FROM "Playlist""#).fetch(executor)
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
        sqlx::query_as(r#"SELECT * FROM "PlaylistEntity""#).fetch(executor)
    }

    /// Fetches all items of a list asynchronously.
    ///
    /// In no particular order.
    #[must_use]
    pub fn fetch_list<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
        list_id: PlaylistId,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(r#"SELECT * FROM "PlaylistEntity" where "listId"=?1"#)
            .bind(list_id)
            .fetch(executor)
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
