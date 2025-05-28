// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use sqlx::FromRow;

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
