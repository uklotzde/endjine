// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use futures_util::stream::BoxStream;
use sqlx::{FromRow, SqlitePool};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Smartlist {
    pub list_uuid: Uuid,
    pub title: String,
    pub parent_playlist_path: String,
    pub next_playlist_path: String,
    pub next_list_uuid: Option<Uuid>,
    pub rules: serde_json::Value,
    pub last_edit_time: OffsetDateTime,
}

/// Fetches all [`Smartlist`]s asynchronously.
///
/// Unfiltered and in no particular order.
#[must_use]
pub fn smartlist_fetch_all(pool: &SqlitePool) -> BoxStream<'_, sqlx::Result<Smartlist>> {
    sqlx::query_as(r"SELECT * FROM Smartlist").fetch(pool)
}

/// Loads a single information by id.
///
/// Returns `Ok(None)` if the requested information has not been found.
pub async fn smartlist_try_load(
    pool: &SqlitePool,
    list_uuid: &Uuid,
) -> sqlx::Result<Option<Smartlist>> {
    sqlx::query_as(r"SELECT * FROM Information WHERE id=?1")
        .bind(list_uuid)
        .fetch_optional(pool)
        .await
}
