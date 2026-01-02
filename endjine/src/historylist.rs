// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use futures_util::stream::BoxStream;
use sqlx::{FromRow, SqliteExecutor};

use crate::{DbUuid, TrackId, UnixTimestamp};

crate::db_id!(HistorylistId);

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Historylist {
    pub id: HistorylistId,
    pub session_id: i64,
    pub title: Option<String>,
    pub start_time: UnixTimestamp,
    pub timezone: Option<String>,
    pub origin_drive_name: Option<String>,
    pub origin_database_id: Option<DbUuid>,
    pub origin_list_id: Option<i64>,
    pub is_deleted: bool,
    pub edit_time: Option<UnixTimestamp>,
}

impl Historylist {
    /// Checks if the table is available in the database.
    pub async fn is_available<'a>(executor: impl SqliteExecutor<'a> + 'a) -> sqlx::Result<bool> {
        let (exists,) = sqlx::query_as(
            r#"SELECT EXISTS(SELECT 1 FROM "sqlite_master" WHERE "type"='table' AND "name"='Historylist')"#,
        )
        .fetch_one(executor)
        .await?;
        Ok(exists)
    }

    /// Fetches all [`Historylist`]s asynchronously.
    ///
    /// Unfiltered and in no particular order.
    #[must_use]
    pub fn fetch_all<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(r#"SELECT * FROM "Historylist" ORDER BY "id""#).fetch(executor)
    }

    /// Loads a single [`Historylist`] by ID.
    ///
    /// Returns `Ok(None)` if the requested [`Historylist`] has not been found.
    pub async fn try_load(
        executor: impl SqliteExecutor<'_>,
        id: HistorylistId,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as(r#"SELECT * FROM "Historylist" WHERE "id"=?1"#)
            .bind(id)
            .fetch_optional(executor)
            .await
    }
}

crate::db_id!(HistorylistEntityId);

/// Entry in a [`Historylist`].
///
/// The terminology used in the schema is confusing and the table
/// should have been named `HistorylistEntry` instead of `HistorylistEntity`.
#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct HistorylistEntity {
    pub id: HistorylistEntityId,
    pub list_id: HistorylistId,
    pub track_id: TrackId,
    pub start_time: UnixTimestamp,
}

impl HistorylistEntity {
    /// Fetches all [`HistorylistEntity`]s asynchronously.
    ///
    /// Unfiltered and in no particular order.
    #[must_use]
    pub fn fetch_all<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(r#"SELECT * FROM "HistorylistEntity" ORDER BY "id""#).fetch(executor)
    }

    /// Fetches all items of a list asynchronously.
    ///
    /// In no particular order.
    #[must_use]
    pub fn fetch_list<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
        list_id: HistorylistId,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(r#"SELECT * FROM "HistorylistEntity" where "listId"=?1"#)
            .bind(list_id)
            .fetch(executor)
    }

    /// Loads a single [`HistorylistEntity`] by ID.
    ///
    /// Returns `Ok(None)` if the requested [`HistorylistEntity`] has not been found.
    pub async fn try_load(
        executor: impl SqliteExecutor<'_>,
        id: HistorylistEntityId,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as(r#"SELECT * FROM "HistorylistEntity" WHERE "id"=?1"#)
            .bind(id)
            .fetch_optional(executor)
            .await
    }
}
