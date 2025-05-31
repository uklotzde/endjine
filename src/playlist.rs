// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use futures_util::stream::BoxStream;
use sqlx::{FromRow, SqliteExecutor, types::time::OffsetDateTime};

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
    pub last_edit_time: OffsetDateTime,
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
        sqlx::query_as(r"SELECT * FROM Playlist").fetch(executor)
    }

    /// Loads a single [`Playlist`] by ID.
    ///
    /// Returns `Ok(None)` if the requested [`Playlist`] has not been found.
    pub async fn try_load(
        executor: impl SqliteExecutor<'_>,
        id: PlaylistId,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as(r"SELECT * FROM Playlist WHERE id=?1")
            .bind(id)
            .fetch_optional(executor)
            .await
    }
}

crate::db_id!(PlaylistEntryId);

/// Entry in a [`Playlist`].
///
/// The terminology used in the schema is confusing and the table
/// should have been named `PlaylistEntry` instead of `PlaylistEntity`.
#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PlaylistEntry {
    pub id: PlaylistEntryId,
    pub list_id: PlaylistId,
    pub track_id: TrackId,
    pub database_uuid: DbUuid,
    pub next_entity_id: PlaylistEntryId,
    pub membership_reference: i64,
}

impl PlaylistEntry {
    /// Fetches all [`PlaylistEntry`]s asynchronously.
    ///
    /// Unfiltered and in no particular order.
    #[must_use]
    pub fn fetch_all<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(r"SELECT * FROM PlaylistEntity").fetch(executor)
    }

    /// Fetches all items of a list asynchronously.
    ///
    /// In no particular order.
    #[must_use]
    pub fn fetch_list<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
        list_id: PlaylistId,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(r"SELECT * FROM PlaylistEntity where listId=?1")
            .bind(list_id)
            .fetch(executor)
    }

    /// Loads a single [`PlaylistEntry`] by ID.
    ///
    /// Returns `Ok(None)` if the requested [`PlaylistEntry`] has not been found.
    pub async fn try_load(
        executor: impl SqliteExecutor<'_>,
        id: PlaylistEntryId,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as(r"SELECT * FROM PlaylistEntity WHERE id=?1")
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
