//! Debug formatting helpers for [`custom_debug_derive`].

use std::fmt;

/// Formats an `Option<T>` by printing the inner value directly (no `Some(...)` wrapper).
///
/// Use with `#[debug(with = "crate::fmt::opt")]` on fields that are
/// already gated by `#[debug(skip_if = Option::is_none)]`.
#[allow(dead_code)]
pub fn opt<T: fmt::Debug>(value: &Option<T>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match value {
        Some(inner) => fmt::Debug::fmt(inner, f),
        None => f.write_str("None"),
    }
}
