// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use futures_util::stream::BoxStream;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, types::time::OffsetDateTime};

crate::db_uuid!(SmartlistUuid);

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Smartlist {
    pub list_uuid: SmartlistUuid,
    pub title: String,
    pub parent_playlist_path: String,
    pub next_playlist_path: String,
    pub next_list_uuid: SmartlistUuid,
    #[sqlx(json)]
    pub rules: SmartlistRules,
    pub last_edit_time: OffsetDateTime,
}

impl Smartlist {
    /// Fetches all [`Smartlist`]s asynchronously.
    ///
    /// Unfiltered and in no particular order.
    #[must_use]
    pub fn fetch_all(pool: &SqlitePool) -> BoxStream<'_, sqlx::Result<Smartlist>> {
        sqlx::query_as(r"SELECT * FROM Smartlist").fetch(pool)
    }

    /// Loads a single [`Smartlist`]s by UUID.
    ///
    /// Returns `Ok(None)` if the requested [`Smartlist`]s has not been found.
    pub async fn try_load(
        pool: &SqlitePool,
        list_uuid: &SmartlistUuid,
    ) -> sqlx::Result<Option<Smartlist>> {
        sqlx::query_as(r"SELECT * FROM Smartlist WHERE listUuid=?1")
            .bind(list_uuid)
            .fetch_optional(pool)
            .await
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartlistRules {
    #[serde(rename = "match")]
    pub r#match: SmartlistRulesMatch,
    pub rules: Vec<SmartlistRulesItem>,
    pub rv: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartlistRulesItem {
    pub col: String,
    pub con: String,
    pub param: String,
    pub v: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartlistRulesMatch {
    One,
    All,
}
