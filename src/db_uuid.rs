// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use std::{borrow::Cow, str::FromStr};

use sqlx::{
    Decode, Encode, Sqlite, Type,
    sqlite::{SqliteTypeInfo, SqliteValueRef},
    types::{Uuid, uuid::fmt::Hyphenated},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DbUuid(Hyphenated);

impl DbUuid {
    #[must_use]
    pub const fn nil() -> Self {
        Self(Hyphenated::from_uuid(Uuid::nil()))
    }

    #[must_use]
    pub const fn is_nil(&self) -> bool {
        self.0.as_uuid().is_nil()
    }

    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        self.0.as_uuid()
    }
}

impl sqlx::Type<Sqlite> for DbUuid {
    fn type_info() -> SqliteTypeInfo {
        <Hyphenated as sqlx::Type<Sqlite>>::type_info()
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <Hyphenated as Type<Sqlite>>::compatible(ty)
    }
}

impl<'r> Decode<'r, Sqlite> for DbUuid {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let value = <Cow<'r, str> as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        if value.is_empty() {
            // Special case: Decode empty string as nil.
            return Ok(Self::nil());
        }
        let uuid = Uuid::from_str(&value)?;
        Ok(Self(Hyphenated::from_uuid(uuid)))
    }
}

impl<'q> Encode<'q, Sqlite> for DbUuid {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        if self.is_nil() {
            // Special case: Encode nil as empty string.
            return <String as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(&String::new(), buf);
        }
        <Hyphenated as Encode<Sqlite>>::encode_by_ref(&self.0, buf)
    }
}
