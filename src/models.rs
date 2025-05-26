// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use sqlx::FromRow;
use time::UtcDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Pack {
    pub id: i64,
    pub pack_id: Option<Uuid>,
    pub change_log_database_uuid: Option<Uuid>,
    pub change_log_id: Option<i64>,
    pub last_pack_time: Option<UtcDateTime>,
}

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Reverse-engineered from database schema."
)]
pub struct Track {
    pub id: i64,
    pub play_order: Option<i64>,
    pub length: Option<i64>,
    pub bpm: Option<i64>,
    pub year: Option<i64>,
    pub path: Option<String>,
    pub filename: Option<String>,
    pub bitrate: Option<i64>,
    pub bpm_analyzed: Option<f64>,
    pub album_art_id: Option<i64>,
    pub file_bytes: Option<i64>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub comment: Option<String>,
    pub label: Option<String>,
    pub composer: Option<String>,
    pub remixer: Option<String>,
    pub key: Option<i64>,
    pub rating: Option<i64>,
    pub album_art: Option<String>,
    pub time_last_played: Option<UtcDateTime>,
    pub is_played: bool,
    pub file_type: Option<String>,
    pub is_analyzed: bool,
    pub date_created: Option<UtcDateTime>,
    pub date_added: Option<UtcDateTime>,
    pub is_available: bool,
    pub is_metadata_of_packed_track_changed: bool,
    pub is_performance_data_of_packed_track_changed: bool,
    pub played_indicator: Option<i64>,
    pub is_metadata_imported: bool,
    pub pdb_import_key: Option<i64>,
    pub streaming_source: Option<String>,
    pub uri: Option<String>,
    pub is_beat_grid_locked: bool,
    pub origin_database_uuid: Option<Uuid>,
    pub origin_track_id: Option<i64>,
    pub streaming_flags: i64,
    pub explicit_lyrics: bool,
    pub last_edit_time: UtcDateTime,
}

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PerformanceData {
    pub track_id: i64,
    pub track_data: Option<Vec<u8>>,
    pub overview_wave_form_data: Option<Vec<u8>>,
    pub beat_data: Option<Vec<u8>>,
    pub quick_cues: Option<Vec<u8>>,
    pub loops: Option<Vec<u8>>,
    pub third_party_source_id: Option<i64>,
    pub active_on_load_loops: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Playlist {
    pub id: i64,
    pub title: String,
    pub parent_list_id: Option<i64>,
    pub is_persisted: bool,
    pub next_list_id: Option<i64>,
    pub last_edit_time: UtcDateTime,
    pub is_explicitly_exported: bool,
}

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PlaylistEntity {
    pub id: i64,
    pub list_id: i64,
    pub track_id: i64,
    pub database_uuid: Option<Uuid>,
    pub next_entity_id: Option<i64>,
    pub membership_reference: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PreparelistEntity {
    pub id: i64,
    pub track_id: i64,
    pub track_number: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Smartlist {
    pub list_uuid: Uuid,
    pub title: String,
    pub parent_playlist_path: Option<String>,
    pub next_playlist_path: Option<String>,
    pub next_list_uuid: Option<Uuid>,
    pub rules: String,
    pub last_edit_time: UtcDateTime,
}
