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
    ) -> BoxStream<'a, sqlx::Result<Playlist>> {
        sqlx::query_as(r"SELECT * FROM Playlist").fetch(executor)
    }

    /// Loads a single [`Playlist`]s by ID.
    ///
    /// Returns `Ok(None)` if the requested [`Playlist`]s has not been found.
    pub async fn try_load(
        executor: impl SqliteExecutor<'_>,
        id: PlaylistId,
    ) -> sqlx::Result<Option<Playlist>> {
        sqlx::query_as(r"SELECT * FROM Playlist WHERE id=?1")
            .bind(id)
            .fetch_optional(executor)
            .await
    }
}

crate::db_id!(PlaylistEntityId);

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
    ) -> BoxStream<'a, sqlx::Result<PlaylistEntity>> {
        sqlx::query_as(r"SELECT * FROM PlaylistEntity").fetch(executor)
    }

    /// Loads a single [`PlaylistEntity`]s by ID.
    ///
    /// Returns `Ok(None)` if the requested [`PlaylistEntity`]s has not been found.
    pub async fn try_load(
        executor: impl SqliteExecutor<'_>,
        id: PlaylistEntityId,
    ) -> sqlx::Result<Option<PlaylistEntity>> {
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
