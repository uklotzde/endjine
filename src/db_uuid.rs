// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

/// Macro for defining type-safe database UUID wrappers for _SQLx_.
///
/// This macro creates a newtype wrapper for defining type-safe database UUIDs.
#[allow(clippy::doc_markdown, reason = "SQLx")]
#[macro_export]
macro_rules! db_uuid {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(transparent)]
        pub struct $name(sqlx::types::uuid::fmt::Hyphenated);

        impl $name {
            #[must_use]
            pub const fn nil() -> Self {
                Self(sqlx::types::uuid::fmt::Hyphenated::from_uuid(
                    sqlx::types::Uuid::nil(),
                ))
            }

            #[must_use]
            pub const fn is_nil(&self) -> bool {
                self.0.as_uuid().is_nil()
            }

            #[must_use]
            pub const fn as_uuid(&self) -> &sqlx::types::Uuid {
                self.0.as_uuid()
            }
        }

        impl sqlx::Type<sqlx::Sqlite> for $name {
            fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
                <sqlx::types::uuid::fmt::Hyphenated as sqlx::Type<sqlx::Sqlite>>::type_info()
            }

            fn compatible(ty: &sqlx::sqlite::SqliteTypeInfo) -> bool {
                <sqlx::types::uuid::fmt::Hyphenated as sqlx::Type<sqlx::Sqlite>>::compatible(ty)
            }
        }

        impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for $name {
            fn decode(
                value: sqlx::sqlite::SqliteValueRef<'r>,
            ) -> Result<Self, sqlx::error::BoxDynError> {
                let value =
                    <std::borrow::Cow<'r, str> as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
                if value.is_empty() {
                    // Special case: Decode empty string as nil.
                    return Ok(Self::nil());
                }
                let uuid = sqlx::types::Uuid::parse_str(&value)?;
                Ok(Self(sqlx::types::uuid::fmt::Hyphenated::from_uuid(uuid)))
            }
        }

        impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
            ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                if self.is_nil() {
                    // Special case: Encode nil as empty string.
                    return <String as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(
                        &String::new(),
                        buf,
                    );
                }
                <sqlx::types::uuid::fmt::Hyphenated as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(
                    &self.0, buf,
                )
            }
        }
    };
}

db_uuid!(DbUuid);

#[cfg(test)]
mod tests {
    use crate::DbUuid;

    #[test]
    fn default_is_nil() {
        assert!(DbUuid::default().is_nil());
        assert_eq!(DbUuid::default(), DbUuid::nil());
    }
}
