// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use sqlx::FromRow;

use crate::TrackId;

crate::db_id!(PreparelistEntityId);

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PreparelistEntity {
    pub id: PreparelistEntityId,
    pub track_id: TrackId,
    pub track_number: Option<i64>,
}
