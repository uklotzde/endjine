// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use futures_util::stream::BoxStream;
use sqlx::{
    Decode, Encode, FromRow, Sqlite, SqliteExecutor,
    encode::IsNull,
    error::BoxDynError,
    sqlite::{SqliteArgumentsBuffer, SqliteTypeInfo, SqliteValueRef},
};

crate::db_id!(AlbumArtId);

const BINARY_HASH_LEN: usize = 20;

const LEGACY_HEX_HASH_LEN: usize = 40;

/// Album
///
/// The hash changed from hex string (schema 3.0.1) to bytes (schema 3.0.2).
/// after purging all album art and re-importing from files.
#[derive(Debug, Clone)]
pub enum AlbumArtHash {
    /// 160-bit binary hash value.
    Binary([u8; BINARY_HASH_LEN]),
    /// Hex-encoded hash value.
    LegacyHex([u8; LEGACY_HEX_HASH_LEN]),
}

impl AlbumArtHash {
    #[must_use]
    const fn as_slice(&self) -> &[u8] {
        match self {
            Self::Binary(slice) => slice,
            Self::LegacyHex(slice) => slice,
        }
    }
}

impl sqlx::Type<Sqlite> for AlbumArtHash {
    fn type_info() -> SqliteTypeInfo {
        <&[u8] as sqlx::Type<Sqlite>>::type_info()
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <&[u8] as sqlx::Type<Sqlite>>::compatible(ty)
    }
}

impl<'r> Decode<'r, Sqlite> for AlbumArtHash {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let value = <&[u8] as Decode<'r, Sqlite>>::decode(value)?;
        let decoded = match value.len() {
            BINARY_HASH_LEN => Self::Binary(value.try_into().unwrap()),
            LEGACY_HEX_HASH_LEN => Self::LegacyHex(value.try_into().unwrap()),
            _ => {
                return Err("invalid length".into());
            }
        };
        Ok(decoded)
    }
}

impl Encode<'_, Sqlite> for AlbumArtHash {
    fn encode_by_ref(&self, buf: &mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError> {
        <&[u8] as Encode<'_, Sqlite>>::encode_by_ref(&self.as_slice(), buf)
    }
}

#[derive(Debug, Clone, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct AlbumArt {
    id: AlbumArtId,

    hash: Option<AlbumArtHash>,

    //
    // Album art is stored as JPG images in the file system starting by
    // Engine 5.0.0 (schema 3.0.2). Values in column albumArt become NULL
    // after purging all album art and re-importing from files.
    #[sqlx(rename = "albumArt")]
    #[expect(dead_code)]
    image_data: Option<Vec<u8>>,
}

impl AlbumArt {
    #[must_use]
    pub const fn id(&self) -> AlbumArtId {
        self.id
    }

    #[must_use]
    pub const fn hash(&self) -> Option<&[u8]> {
        if let Some(hash) = &self.hash {
            return Some(hash.as_slice());
        }
        None
    }

    /// Fetches all [`AlbumArt`] asynchronously.
    ///
    /// Unfiltered and in no particular order.
    #[must_use]
    pub fn fetch_all<'a>(
        executor: impl SqliteExecutor<'a> + 'a,
    ) -> BoxStream<'a, sqlx::Result<Self>> {
        sqlx::query_as(r#"SELECT * FROM "AlbumArt" ORDER BY "id""#).fetch(executor)
    }

    /// Loads a single [`AlbumArt`] by id.
    ///
    /// Returns `Ok(None)` if the requested [`AlbumArt`] has not been found.
    pub async fn try_load(
        executor: impl SqliteExecutor<'_>,
        id: AlbumArtId,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as(r#"SELECT * FROM "AlbumArt" WHERE "id"=?1"#)
            .bind(id)
            .fetch_optional(executor)
            .await
    }

    pub async fn delete_unused(executor: impl SqliteExecutor<'_>) -> sqlx::Result<u64> {
        let result =
            sqlx::query(r#"DELETE FROM "AlbumArt" WHERE "id" NOT IN (SELECT "albumArtId" FROM "Track" WHERE "albumArtId" IS NOT NULL)"#)
                .execute(executor)
                .await?;
        Ok(result.rows_affected())
    }
}
