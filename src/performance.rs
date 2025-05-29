// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use futures_util::stream::BoxStream;
use sqlx::{FromRow, SqliteExecutor};

use crate::TrackId;

crate::db_id!(PerformanceDataId);

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PerformanceData {
    pub track_id: TrackId,
    pub track_data: Vec<u8>,
    pub overview_wave_form_data: Vec<u8>,
    pub beat_data: Vec<u8>,
    pub quick_cues: Vec<u8>,
    pub loops: Vec<u8>,
    pub third_party_source_id: Option<i64>,
    pub active_on_load_loops: i64,
}

impl PerformanceData {
    /// Fetches all [`PerformanceData`]s asynchronously.
    ///
    /// Unfiltered and in no particular order.
    #[must_use]
    pub fn fetch_all<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
    ) -> BoxStream<'a, sqlx::Result<PerformanceData>> {
        sqlx::query_as(r"SELECT * FROM PerformanceData").fetch(executor)
    }

    /// Loads a single [`PerformanceData`]s by ID.
    ///
    /// Returns `Ok(None)` if the requested [`PerformanceData`]s has not been found.
    pub async fn try_load(
        executor: impl SqliteExecutor<'_>,
        id: PerformanceDataId,
    ) -> sqlx::Result<Option<PerformanceData>> {
        sqlx::query_as(r"SELECT * FROM PerformanceData WHERE id=?1")
            .bind(id)
            .fetch_optional(executor)
            .await
    }

    /// Delete all records with no associated track.
    pub async fn delete_orphaned(executor: impl SqliteExecutor<'_>) -> sqlx::Result<u64> {
        let result =
            sqlx::query(r"DELETE FROM PerformanceData WHERE trackId NOT IN (SELECT id FROM Track)")
                .execute(executor)
                .await?;
        Ok(result.rows_affected())
    }
}
