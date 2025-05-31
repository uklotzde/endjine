// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use futures_util::stream::BoxStream;
use sqlx::{FromRow, SqliteExecutor};

use crate::TrackId;

crate::db_id!(PreparelistEntryId);

/// Entry in the _Preparelist_.
///
/// The terminology used in the schema is confusing and the table
/// should have been named `PreparelistEntry` instead of `PreparelistEntity`.
#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PreparelistEntry {
    pub id: PreparelistEntryId,
    pub track_id: TrackId,
    pub track_number: i64,
}

impl PreparelistEntry {
    /// Checks if the table is available in the database.
    pub async fn is_available<'a>(executor: impl SqliteExecutor<'a> + 'a) -> sqlx::Result<bool> {
        let (exists,) = sqlx::query_as(
            r"SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='PreparelistEntity')",
        )
        .fetch_one(executor)
        .await?;
        Ok(exists)
    }

    /// Fetches all [`PreparelistEntry`]s asynchronously.
    ///
    /// Unfiltered and in no particular order.
    #[must_use]
    pub fn fetch_all<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(r"SELECT * FROM PreparelistEntity").fetch(executor)
    }

    /// Loads a single [`PreparelistEntry`] by ID.
    ///
    /// Returns `Ok(None)` if the requested [`PreparelistEntry`] has not been found.
    pub async fn try_load(
        executor: impl SqliteExecutor<'_>,
        id: PreparelistEntryId,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as(r"SELECT * FROM PreparelistEntity WHERE id=?1")
            .bind(id)
            .fetch_optional(executor)
            .await
    }
}
