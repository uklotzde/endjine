// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use sqlx::{FromRow, types::time::OffsetDateTime};

use crate::{ChangeLogId, DbUuid};

crate::db_id!(PackId);

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Pack {
    pub id: PackId,
    pub pack_id: DbUuid,
    pub change_log_database_uuid: DbUuid,
    pub change_log_id: ChangeLogId,
    pub last_pack_time: OffsetDateTime,
}
