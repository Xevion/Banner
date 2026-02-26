//! Unsigned integer newtypes for non-negative database fields.
//!
//! PostgreSQL has no unsigned integer types, so we store values as `INTEGER`/`BIGINT`
//! and convert at the Rust boundary. The `unsigned_newtype!` macro generates all
//! necessary trait impls (SQLx, ts-rs, serde, conversions) from a single invocation.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Generate a newtype wrapper around an unsigned integer that maps to a signed
/// PostgreSQL column type. Produces:
///
/// - SQLx `Type`/`Encode`/`Decode` (maps `u32`<->`i32` or `u64`<->`i64`)
/// - ts-rs `TS` (inlines as `"number"`)
/// - `serde` transparent serialization
/// - `Display`, `From<unsigned>`, `Into<unsigned>`
/// - `TryFrom<i32>`, `TryFrom<i64>`, `TryFrom<usize>` for fallible signed conversions
macro_rules! unsigned_newtype {
    ($name:ident, u32) => {
        #[derive(
            Default,
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(u32);

        impl $name {
            pub fn new(val: u32) -> Self {
                Self(val)
            }

            pub fn get(self) -> u32 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl From<u32> for $name {
            fn from(val: u32) -> Self {
                Self(val)
            }
        }

        impl From<$name> for u32 {
            fn from(val: $name) -> Self {
                val.0
            }
        }

        impl TryFrom<i32> for $name {
            type Error = std::num::TryFromIntError;
            fn try_from(val: i32) -> Result<Self, Self::Error> {
                u32::try_from(val).map(Self)
            }
        }

        impl TryFrom<i64> for $name {
            type Error = std::num::TryFromIntError;
            fn try_from(val: i64) -> Result<Self, Self::Error> {
                u32::try_from(val).map(Self)
            }
        }

        impl TryFrom<usize> for $name {
            type Error = std::num::TryFromIntError;
            fn try_from(val: usize) -> Result<Self, Self::Error> {
                u32::try_from(val).map(Self)
            }
        }

        // SQLx: map to INTEGER (i32's Postgres type)
        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <i32 as sqlx::Type<sqlx::Postgres>>::type_info()
            }
        }

        impl sqlx::Encode<'_, sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut sqlx::postgres::PgArgumentBuffer,
            ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                let v = i32::try_from(self.0)
                    .map_err(|_| format!("{} value {} overflows i32", stringify!($name), self.0))?;
                <i32 as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&v, buf)
            }
        }

        impl<'r> sqlx::Decode<'r, sqlx::Postgres> for $name {
            fn decode(
                value: sqlx::postgres::PgValueRef<'r>,
            ) -> Result<Self, sqlx::error::BoxDynError> {
                let raw = <i32 as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
                u32::try_from(raw).map(Self).map_err(|_| {
                    format!(
                        "negative i32 {} cannot decode as {}",
                        raw,
                        stringify!($name)
                    )
                    .into()
                })
            }
        }

        // ts-rs: inline as "number" with no exported .ts file
        impl ts_rs::TS for $name {
            type WithoutGenerics = Self;
            type OptionInnerType = Self;

            fn name() -> String {
                "number".to_owned()
            }

            fn inline() -> String {
                "number".to_owned()
            }

            fn decl() -> String {
                panic!("{} cannot be declared", stringify!($name))
            }

            fn decl_concrete() -> String {
                panic!("{} cannot be declared", stringify!($name))
            }

            fn inline_flattened() -> String {
                panic!("{} cannot be flattened", stringify!($name))
            }
        }
    };

    ($name:ident, u64) => {
        #[derive(
            Default,
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(u64);

        impl $name {
            pub fn new(val: u64) -> Self {
                Self(val)
            }

            pub fn get(self) -> u64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl From<u64> for $name {
            fn from(val: u64) -> Self {
                Self(val)
            }
        }

        impl From<$name> for u64 {
            fn from(val: $name) -> Self {
                val.0
            }
        }

        impl TryFrom<i32> for $name {
            type Error = std::num::TryFromIntError;
            fn try_from(val: i32) -> Result<Self, Self::Error> {
                u64::try_from(val).map(Self)
            }
        }

        impl TryFrom<i64> for $name {
            type Error = std::num::TryFromIntError;
            fn try_from(val: i64) -> Result<Self, Self::Error> {
                u64::try_from(val).map(Self)
            }
        }

        impl TryFrom<usize> for $name {
            type Error = std::num::TryFromIntError;
            fn try_from(val: usize) -> Result<Self, Self::Error> {
                u64::try_from(val).map(Self)
            }
        }

        // SQLx: map to BIGINT (i64's Postgres type)
        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <i64 as sqlx::Type<sqlx::Postgres>>::type_info()
            }
        }

        impl sqlx::Encode<'_, sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut sqlx::postgres::PgArgumentBuffer,
            ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                let v = i64::try_from(self.0)
                    .map_err(|_| format!("{} value {} overflows i64", stringify!($name), self.0))?;
                <i64 as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&v, buf)
            }
        }

        impl<'r> sqlx::Decode<'r, sqlx::Postgres> for $name {
            fn decode(
                value: sqlx::postgres::PgValueRef<'r>,
            ) -> Result<Self, sqlx::error::BoxDynError> {
                let raw = <i64 as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
                u64::try_from(raw).map(Self).map_err(|_| {
                    format!(
                        "negative i64 {} cannot decode as {}",
                        raw,
                        stringify!($name)
                    )
                    .into()
                })
            }
        }

        // ts-rs: inline as "number" with no exported .ts file
        impl ts_rs::TS for $name {
            type WithoutGenerics = Self;
            type OptionInnerType = Self;

            fn name() -> String {
                "number".to_owned()
            }

            fn inline() -> String {
                "number".to_owned()
            }

            fn decl() -> String {
                panic!("{} cannot be declared", stringify!($name))
            }

            fn decl_concrete() -> String {
                panic!("{} cannot be declared", stringify!($name))
            }

            fn inline_flattened() -> String {
                panic!("{} cannot be flattened", stringify!($name))
            }
        }
    };
}

unsigned_newtype!(Count, u32);
unsigned_newtype!(DurationMs, u32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_construction() {
        let c = Count::new(42);
        assert_eq!(c.get(), 42);
        assert_eq!(u32::from(c), 42);

        let c2: Count = 10u32.into();
        assert_eq!(c2.get(), 10);
    }

    #[test]
    fn count_try_from_signed() {
        assert_eq!(Count::try_from(0i32).unwrap().get(), 0);
        assert_eq!(Count::try_from(100i32).unwrap().get(), 100);
        assert!(Count::try_from(-1i32).is_err());

        assert_eq!(Count::try_from(0i64).unwrap().get(), 0);
        assert!(Count::try_from(-1i64).is_err());
        assert!(Count::try_from(i64::from(u32::MAX) + 1).is_err());
    }

    #[test]
    fn count_try_from_usize() {
        assert_eq!(Count::try_from(0usize).unwrap().get(), 0);
        assert_eq!(Count::try_from(100usize).unwrap().get(), 100);
    }

    #[test]
    fn count_display() {
        assert_eq!(format!("{}", Count::new(42)), "42");
    }

    #[test]
    fn count_serde_transparent() {
        let c = Count::new(42);
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, "42");

        let deserialized: Count = serde_json::from_str("42").unwrap();
        assert_eq!(deserialized.get(), 42);
    }

    #[test]
    fn count_ts_inlines_as_number() {
        assert_eq!(<Count as ts_rs::TS>::name(), "number");
        assert_eq!(<Count as ts_rs::TS>::inline(), "number");
    }

    #[test]
    fn duration_ms_construction() {
        let d = DurationMs::new(1500);
        assert_eq!(d.get(), 1500);
    }

    #[test]
    fn duration_ms_ts_inlines_as_number() {
        assert_eq!(<DurationMs as ts_rs::TS>::name(), "number");
        assert_eq!(<DurationMs as ts_rs::TS>::inline(), "number");
    }

    #[test]
    fn count_ordering() {
        let a = Count::new(1);
        let b = Count::new(2);
        assert!(a < b);
        assert_eq!(a, Count::new(1));
    }

    #[test]
    fn count_encode_overflow() {
        // u32::MAX (4294967295) exceeds i32::MAX (2147483647)
        let c = Count::new(u32::MAX);
        let mut buf = sqlx::postgres::PgArgumentBuffer::default();
        let result = sqlx::Encode::<sqlx::Postgres>::encode_by_ref(&c, &mut buf);
        assert!(result.is_err());
    }
}
