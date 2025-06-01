// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

#![allow(unreachable_pub, reason = "False positive?")]

/// Macro for defining type-safe database ID wrappers for _SQLx_.
///
/// This macro creates a newtype wrapper for defining type-safe database IDs.
#[allow(clippy::doc_markdown, reason = "SQLx")]
#[macro_export]
macro_rules! db_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(transparent)]
        pub struct $name(i64);

        impl $name {
            /// Exclusive lower bound.
            ///
            /// Does never occur in a table all valid IDs are strictly positive.
            pub const INVALID_MIN_EXCLUSIVE: Self = Self(-1);

            /// Zero.
            ///
            /// Some columns use the value `0` instead of `NULL`, even though this practice
            /// prevents to define foreign key constraints in the database!? Probably to circumvent
            /// issues around indexing `NULL` values in _SQLite_.
            ///
            /// See also:
            ///   - <https://www.sqlite.org/nulls.html>
            ///   - <https://www.sqlite.org/partialindex.html>
            #[allow(clippy::doc_markdown, reason = "SQLite")]
            pub const INVALID_ZERO: Self = Self(0);

            /// Checks if the ID is valid.
            #[must_use]
            pub const fn is_valid(self) -> bool {
                self.0 > Self::INVALID_ZERO.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        // SQLx integration: Derive implementations using transparent repr
        impl sqlx::Type<sqlx::Sqlite> for $name {
            fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
                <i64 as sqlx::Type<sqlx::Sqlite>>::type_info()
            }

            fn compatible(ty: &sqlx::sqlite::SqliteTypeInfo) -> bool {
                <i64 as sqlx::Type<sqlx::Sqlite>>::compatible(ty)
            }
        }

        impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for $name {
            fn decode(
                value: sqlx::sqlite::SqliteValueRef<'r>,
            ) -> Result<Self, sqlx::error::BoxDynError> {
                let value = <i64 as sqlx::Decode<'r, sqlx::Sqlite>>::decode(value)?;
                let id = Self(value);
                debug_assert!(id.is_valid() || id == Self::INVALID_ZERO);
                Ok(id)
            }
        }

        impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
            ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                <i64 as sqlx::Encode<'q, sqlx::Sqlite>>::encode_by_ref(&self.0, buf)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    db_id!(TestId);

    #[test]
    fn is_valid() {
        assert!(!TestId::INVALID_MIN_EXCLUSIVE.is_valid());
        assert!(!TestId::INVALID_ZERO.is_valid());
        assert!(!TestId(TestId::INVALID_ZERO.0 - 1).is_valid());
        assert!(TestId(TestId::INVALID_ZERO.0 + 1).is_valid());
    }

    #[test]
    fn default_is_invalid_zero() {
        assert_eq!(TestId::default(), TestId::INVALID_ZERO);
    }
}
