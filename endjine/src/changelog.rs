// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use sqlx::FromRow;

use crate::TrackId;

crate::db_id!(ChangeLogId);

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct ChangeLog {
    pub id: ChangeLogId,
    pub track_id: TrackId,
}
