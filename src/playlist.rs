// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use sqlx::FromRow;
use time::UtcDateTime;
use uuid::Uuid;

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
pub struct PlaylistAllChildren {
    pub id: i64,
    pub child_list_id: i64,
}

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PlaylistAllParent {
    pub id: i64,
    pub parent_list_id: i64,
}

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PlaylistPath {
    pub id: i64,
    pub path: String,
    pub position: i64,
}
