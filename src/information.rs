// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::fmt;

use anyhow::bail;
use futures_util::StreamExt as _;
use sqlx::{FromRow, SqliteExecutor};

use crate::DbUuid;

/// Latest schema major version.
///
/// Only the latest schema version is supported.
pub const SCHEMA_VERSION_MAJOR: u32 = 3;

/// Latest schema minor version.
///
/// Only the latest schema version is supported.
pub const SCHEMA_VERSION_MINOR: u32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SchemaVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SchemaVersion {
    #[must_use]
    pub const fn is_supported(&self) -> bool {
        let Self {
            major,
            minor,
            patch: _,
        } = self;
        *major == SCHEMA_VERSION_MAJOR && *minor == SCHEMA_VERSION_MINOR
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            major,
            minor,
            patch,
        } = self;
        write!(f, "{major}.{minor}.{patch}")
    }
}

crate::db_id!(InformationId);

/// Database information.
#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Information {
    id: InformationId,
    uuid: DbUuid,
    schema_version_major: i64,
    schema_version_minor: i64,
    schema_version_patch: i64,
    // Typo in column name of database schema requires renaming.
    #[sqlx(rename = "currentPlayedIndiciator")]
    #[expect(dead_code, reason = "Not used yet.")]
    current_played_indicator: Option<i64>,
    #[expect(dead_code, reason = "Not used yet.")]
    last_rekord_box_library_import_read_counter: Option<i64>,
}

impl Information {
    #[must_use]
    pub const fn id(&self) -> InformationId {
        self.id
    }

    #[must_use]
    pub const fn uuid(&self) -> &DbUuid {
        &self.uuid
    }

    /// Gets the schema version.
    ///
    /// # Panics
    ///
    /// Panics if any of the version numbers is negative or exceeds the maximum value.
    #[must_use]
    pub fn schema_version(&self) -> SchemaVersion {
        let major = self
            .schema_version_major
            .try_into()
            .expect("valid major number");
        let minor = self
            .schema_version_minor
            .try_into()
            .expect("valid minor number");
        let patch = self
            .schema_version_patch
            .try_into()
            .expect("valid patch number");
        SchemaVersion {
            major,
            minor,
            patch,
        }
    }

    pub async fn count_all(executor: impl SqliteExecutor<'_>) -> sqlx::Result<u64> {
        let count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM "Information""#)
            .fetch_one(executor)
            .await?;
        debug_assert!(count >= 0);
        Ok(count.cast_unsigned())
    }

    /// Loads the singular entry.
    ///
    /// Fails if the table contains none or more than one entry.
    pub async fn load<'e, E>(mut executor: impl FnMut() -> E) -> anyhow::Result<Self>
    where
        E: SqliteExecutor<'e>,
    {
        let mut row_results =
            sqlx::query_as(r#"SELECT * FROM "Information" LIMIT 2"#).fetch(executor());
        let Some(row_result) = row_results.next().await else {
            // Table is empty.
            debug_assert_eq!(Self::count_all(executor()).await.ok(), Some(0));
            return Err(sqlx::Error::RowNotFound.into());
        };
        let row = row_result?;
        if row_results.next().await.is_some() {
            bail!("ambiguous");
        }
        Ok(row)
    }

    /// Eagerly loads all [`Information`] at once.
    ///
    /// Unfiltered and in no particular order.
    pub async fn load_all(executor: impl SqliteExecutor<'_>) -> sqlx::Result<Vec<Self>> {
        sqlx::query_as(r#"SELECT * FROM "Information""#)
            .fetch_all(executor)
            .await
    }

    /// Loads a single [`Information`] by id.
    ///
    /// Returns `Ok(None)` if the requested [`Information`] has not been found.
    pub async fn try_load(
        executor: impl SqliteExecutor<'_>,
        id: InformationId,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as(r#"SELECT * FROM "Information" WHERE "id"=?1"#)
            .bind(id)
            .fetch_optional(executor)
            .await
    }

    /// Loads a single [`Information`] by UUID.
    ///
    /// Returns `Ok(None)` if the requested [`Information`] has not been found.
    pub async fn try_load_by_uuid(
        executor: impl SqliteExecutor<'_>,
        uuid: &DbUuid,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as(r#"SELECT * FROM "Information" WHERE "uuid"=?1"#)
            .bind(uuid)
            .fetch_optional(executor)
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::{SCHEMA_VERSION_MAJOR, SCHEMA_VERSION_MINOR, information::SchemaVersion};

    #[test]
    fn schema_version_supported() {
        assert!(
            SchemaVersion {
                major: SCHEMA_VERSION_MAJOR,
                minor: SCHEMA_VERSION_MINOR,
                patch: u32::MIN
            }
            .is_supported()
        );
        assert!(
            SchemaVersion {
                major: SCHEMA_VERSION_MAJOR,
                minor: SCHEMA_VERSION_MINOR,
                patch: u32::MAX
            }
            .is_supported()
        );
        assert!(
            !SchemaVersion {
                major: SCHEMA_VERSION_MAJOR.checked_sub(1).unwrap(),
                minor: SCHEMA_VERSION_MINOR,
                patch: u32::MIN
            }
            .is_supported()
        );
        assert!(
            !SchemaVersion {
                major: SCHEMA_VERSION_MAJOR.checked_add(1).unwrap(),
                minor: SCHEMA_VERSION_MINOR,
                patch: u32::MIN
            }
            .is_supported()
        );
        assert!(
            !SchemaVersion {
                major: SCHEMA_VERSION_MAJOR,
                minor: SCHEMA_VERSION_MINOR.checked_add(1).unwrap(),
                patch: u32::MIN
            }
            .is_supported()
        );
    }
}
