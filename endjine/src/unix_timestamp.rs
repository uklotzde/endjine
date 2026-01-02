// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use sqlx::{
    Decode, Encode, Sqlite,
    encode::IsNull,
    error::BoxDynError,
    sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef},
    types::time::OffsetDateTime,
};

/// UNIX timestamp.
///
/// Encoded as integer seconds since epoch origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct UnixTimestamp {
    pub seconds_since_epoch_origin: i64,
}

// SQLx integration: Derive implementations using transparent repr
impl sqlx::Type<Sqlite> for UnixTimestamp {
    fn type_info() -> SqliteTypeInfo {
        <i64 as sqlx::Type<Sqlite>>::type_info()
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <OffsetDateTime as sqlx::Type<Sqlite>>::compatible(ty)
    }
}

impl<'r> Decode<'r, Sqlite> for UnixTimestamp {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let ts = <OffsetDateTime as Decode<'r, Sqlite>>::decode(value)?;
        let seconds_since_epoch_origin = ts.unix_timestamp();
        Ok(Self {
            seconds_since_epoch_origin,
        })
    }
}

impl<'q> Encode<'q, Sqlite> for UnixTimestamp {
    fn encode_by_ref(&self, buf: &mut Vec<SqliteArgumentValue<'q>>) -> Result<IsNull, BoxDynError> {
        let Self {
            seconds_since_epoch_origin,
        } = self;
        <i64 as Encode<Sqlite>>::encode_by_ref(seconds_since_epoch_origin, buf)
    }
}
