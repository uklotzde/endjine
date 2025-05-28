// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PreparelistEntity {
    pub id: i64,
    pub track_id: i64,
    pub track_number: Option<i64>,
}
