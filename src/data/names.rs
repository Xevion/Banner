//! Name parsing, normalization, and matching utilities.
//!
//! Handles the mismatch between Banner's single `display_name` ("Last, First Middle")
//! and RMP's separate `first_name`/`last_name` fields, plus data quality issues
//! from both sources (HTML entities, accents, nicknames, suffixes, junk).

use sqlx::PgPool;
use tracing::{info, warn};
use unicode_normalization::UnicodeNormalization;

/// Known name suffixes to extract from the last-name portion.
const SUFFIXES: &[&str] = &["iv", "iii", "ii", "jr", "sr"];

/// Parsed, cleaned name components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameParts {
    /// Cleaned display-quality first name(s): "H. Paul", "María"
    pub first: String,
    /// Cleaned display-quality last name: "O'Brien", "LeBlanc"
    pub last: String,
    /// Middle name/initial if detected: "Manuel", "L."
    pub middle: Option<String>,
    /// Suffix if detected: "III", "Jr"
    pub suffix: Option<String>,
    /// Nicknames extracted from parentheses: ["Ken"], ["Qian"]
    pub nicknames: Vec<String>,
}

/// Decode common HTML entities found in Banner data.
///
/// Handles both named entities (`&amp;`, `&uuml;`) and numeric references
/// (`&#39;`, `&#x27;`).
pub(crate) fn decode_html_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    htmlize::unescape(s).to_string()
}

/// Extract parenthesized nicknames from a name string.
///
/// `"William (Ken)"` → `("William", vec!["Ken"])`
/// `"Guenevere (Qian)"` → `("Guenevere", vec!["Qian"])`
/// `"John (jack) C."` → `("John C.", vec!["jack"])`
fn extract_nicknames(s: &str) -> (String, Vec<String>) {
    let mut nicknames = Vec::new();
    let mut cleaned = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '(' {
            let mut nick = String::new();
            for inner in chars.by_ref() {
                if inner == ')' {
                    break;
                }
                nick.push(inner);
            }
            let nick = nick.trim().to_string();
            if !nick.is_empty() {
                nicknames.push(nick);
            }
        } else if ch == '"' || ch == '\u{201C}' || ch == '\u{201D}' {
            // Extract quoted nicknames: Thomas "Butch" → nickname "Butch"
            let mut nick = String::new();
            for inner in chars.by_ref() {
                if inner == '"' || inner == '\u{201C}' || inner == '\u{201D}' {
                    break;
                }
                nick.push(inner);
            }
            let nick = nick.trim().to_string();
            if !nick.is_empty() {
                nicknames.push(nick);
            }
        } else {
            cleaned.push(ch);
        }
    }

    // Collapse multiple spaces left by extraction
    let cleaned = collapse_whitespace(&cleaned);
    (cleaned, nicknames)
}

/// Extract a suffix (Jr, Sr, II, III, IV) from the last-name portion.
///
/// `"LeBlanc III"` → `("LeBlanc", Some("III"))`
/// `"Smith Jr."` → `("Smith", Some("Jr."))`
fn extract_suffix(last: &str) -> (String, Option<String>) {
    // Try to match the last token as a suffix
    let tokens: Vec<&str> = last.split_whitespace().collect();
    if tokens.len() < 2 {
        return (last.to_string(), None);
    }

    let candidate = tokens.last().unwrap();
    let candidate_normalized = candidate.to_lowercase().trim_end_matches('.').to_string();

    if SUFFIXES.contains(&candidate_normalized.as_str()) {
        let name_part = tokens[..tokens.len() - 1].join(" ");
        return (name_part, Some(candidate.to_string()));
    }

    (last.to_string(), None)
}

/// Strip junk commonly found in RMP name fields.
///
/// - Trailing commas: `"Cronenberger,"` → `"Cronenberger"`
/// - Email addresses: `"Neel.Baumgardner@utsa.edu"` → `""` (returns empty)
fn strip_junk(s: &str) -> String {
    let s = s.trim();

    // If the string looks like an email, return empty
    if s.contains('@') && s.contains('.') && !s.contains(' ') {
        return String::new();
    }

    // Strip trailing commas
    s.trim_end_matches(',').trim().to_string()
}

/// Collapse runs of whitespace into single spaces and trim.
fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse a Banner `display_name` ("Last, First Middle") into structured parts.
///
/// Handles HTML entities, suffixes, and multi-token names.
///
/// # Examples
///
/// ```
/// use banner::data::names::parse_banner_name;
///
/// let parts = parse_banner_name("O&#39;Brien, Erin").unwrap();
/// assert_eq!(parts.first, "Erin");
/// assert_eq!(parts.last, "O'Brien");
/// ```
pub fn parse_banner_name(display_name: &str) -> Option<NameParts> {
    // 1. Decode HTML entities
    let decoded = decode_html_entities(display_name);

    // 2. Split on first comma
    let (last_part, first_part) = decoded.split_once(',')?;
    let last_part = last_part.trim();
    let first_part = first_part.trim();

    if last_part.is_empty() || first_part.is_empty() {
        return None;
    }

    // 3. Extract suffix from last name
    let (last_clean, suffix) = extract_suffix(last_part);

    // 4. Parse first-name portion: first token(s) + optional middle
    // Banner format is "First Middle", so we keep all tokens as first_name
    // to support "H. Paul" style names
    let first_clean = collapse_whitespace(first_part);

    Some(NameParts {
        first: first_clean,
        last: last_clean,
        middle: None, // Banner doesn't clearly delineate middle vs first
        suffix,
        nicknames: Vec::new(), // Banner doesn't include nicknames
    })
}

/// Parse RMP professor name fields into structured parts.
///
/// Handles junk data, nicknames in parentheses/quotes, and suffixes.
///
/// # Examples
///
/// ```
/// use banner::data::names::parse_rmp_name;
///
/// let parts = parse_rmp_name("William (Ken)", "Burchenal").unwrap();
/// assert_eq!(parts.first, "William");
/// assert_eq!(parts.nicknames, vec!["Ken"]);
/// ```
pub fn parse_rmp_name(first_name: &str, last_name: &str) -> Option<NameParts> {
    let first_cleaned = strip_junk(first_name);
    let last_cleaned = strip_junk(last_name);

    if first_cleaned.is_empty() || last_cleaned.is_empty() {
        return None;
    }

    // Extract nicknames from parens/quotes in first name
    let (first_no_nicks, nicknames) = extract_nicknames(&first_cleaned);
    let first_final = collapse_whitespace(&first_no_nicks);

    // Extract suffix from last name
    let (last_final, suffix) = extract_suffix(&last_cleaned);

    if first_final.is_empty() || last_final.is_empty() {
        return None;
    }

    Some(NameParts {
        first: first_final,
        last: last_final,
        middle: None,
        suffix,
        nicknames,
    })
}

/// Normalize a name string for matching comparison.
///
/// Pipeline: lowercase → NFD decompose → strip combining marks →
/// strip punctuation/hyphens → collapse whitespace → trim.
///
/// # Examples
///
/// ```
/// use banner::data::names::normalize_for_matching;
///
/// assert_eq!(normalize_for_matching("García"), "garcia");
/// assert_eq!(normalize_for_matching("O'Brien"), "obrien");
/// assert_eq!(normalize_for_matching("Aguirre-Mesa"), "aguirremesa");
/// ```
/// Normalize a name string for matching index keys.
///
/// Pipeline: lowercase → NFD decompose → strip combining marks →
/// strip ALL punctuation, hyphens, and whitespace.
///
/// This produces a compact, space-free string so that "Aguirre Mesa" (Banner)
/// and "Aguirre-Mesa" (RMP) both become "aguirremesa".
///
/// # Examples
///
/// ```
/// use banner::data::names::normalize_for_matching;
///
/// assert_eq!(normalize_for_matching("García"), "garcia");
/// assert_eq!(normalize_for_matching("O'Brien"), "obrien");
/// assert_eq!(normalize_for_matching("Aguirre-Mesa"), "aguirremesa");
/// assert_eq!(normalize_for_matching("Aguirre Mesa"), "aguirremesa");
/// ```
pub fn normalize_for_matching(s: &str) -> String {
    s.to_lowercase()
        .nfd()
        .filter(|c| {
            // Keep only non-combining alphabetic characters — strip everything else
            c.is_alphabetic() && !unicode_normalization::char::is_combining_mark(*c)
        })
        .collect()
}

/// Generate all matching index keys for a parsed name.
///
/// For a name like "H. Paul" / "LeBlanc" with no nicknames, generates:
/// - `("leblanc", "h paul")` — full normalized first
/// - `("leblanc", "paul")` — individual token (if multi-token)
/// - `("leblanc", "h")` — individual token (if multi-token)
///
/// For a name like "William" / "Burchenal" with nickname "Ken":
/// - `("burchenal", "william")` — primary
/// - `("burchenal", "ken")` — nickname variant
pub fn matching_keys(parts: &NameParts) -> Vec<(String, String)> {
    let norm_last = normalize_for_matching(&parts.last);
    if norm_last.is_empty() {
        return Vec::new();
    }

    let mut keys = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Primary key: full first name (all spaces stripped)
    let norm_first_full = normalize_for_matching(&parts.first);
    if !norm_first_full.is_empty() && seen.insert(norm_first_full.clone()) {
        keys.push((norm_last.clone(), norm_first_full));
    }

    // Individual tokens from the display-form first name
    // (split before full normalization so we can generate per-token keys)
    let first_tokens: Vec<&str> = parts.first.split_whitespace().collect();
    if first_tokens.len() > 1 {
        for token in &first_tokens {
            let norm_token = normalize_for_matching(token);
            if !norm_token.is_empty() && seen.insert(norm_token.clone()) {
                keys.push((norm_last.clone(), norm_token));
            }
        }
    }

    // Nickname variants
    for nick in &parts.nicknames {
        let norm_nick = normalize_for_matching(nick);
        if !norm_nick.is_empty() && seen.insert(norm_nick.clone()) {
            keys.push((norm_last.clone(), norm_nick));
        }
    }

    keys
}

/// Backfill `first_name`/`last_name` columns for all instructors that have
/// a `display_name` but NULL structured name fields.
///
/// Parses each `display_name` using [`parse_banner_name`] and updates the row.
/// Logs warnings for any names that fail to parse.
pub async fn backfill_instructor_names(db_pool: &PgPool) -> anyhow::Result<()> {
    let rows: Vec<(i32, String)> = sqlx::query_as(
        "SELECT id, display_name FROM instructors WHERE first_name IS NULL OR last_name IS NULL",
    )
    .fetch_all(db_pool)
    .await?;

    if rows.is_empty() {
        return Ok(());
    }

    let total = rows.len();
    let mut ids: Vec<i32> = Vec::with_capacity(total);
    let mut firsts: Vec<String> = Vec::with_capacity(total);
    let mut lasts: Vec<String> = Vec::with_capacity(total);
    let mut unparseable = 0usize;

    for (id, display_name) in &rows {
        match parse_banner_name(display_name) {
            Some(parts) => {
                ids.push(*id);
                firsts.push(parts.first);
                lasts.push(parts.last);
            }
            None => {
                warn!(
                    id,
                    display_name, "Failed to parse instructor display_name during backfill"
                );
                unparseable += 1;
            }
        }
    }

    if !ids.is_empty() {
        let first_refs: Vec<&str> = firsts.iter().map(|s| s.as_str()).collect();
        let last_refs: Vec<&str> = lasts.iter().map(|s| s.as_str()).collect();

        sqlx::query(
            r#"
            UPDATE instructors i
            SET first_name = v.first_name, last_name = v.last_name
            FROM UNNEST($1::int4[], $2::text[], $3::text[])
                AS v(id, first_name, last_name)
            WHERE i.id = v.id
            "#,
        )
        .bind(&ids)
        .bind(&first_refs)
        .bind(&last_refs)
        .execute(db_pool)
        .await?;
    }

    info!(
        total,
        updated = ids.len(),
        unparseable,
        "Instructor name backfill complete"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // HTML entity decoding
    // -----------------------------------------------------------------------

    #[test]
    fn decode_apostrophe_entity() {
        assert_eq!(decode_html_entities("O&#39;Brien"), "O'Brien");
    }

    #[test]
    fn decode_umlaut_entity() {
        assert_eq!(decode_html_entities("B&uuml;lent"), "Bülent");
    }

    #[test]
    fn decode_no_entities() {
        assert_eq!(decode_html_entities("Smith"), "Smith");
    }

    // -----------------------------------------------------------------------
    // Nickname extraction
    // -----------------------------------------------------------------------

    #[test]
    fn extract_paren_nickname() {
        let (cleaned, nicks) = extract_nicknames("William (Ken)");
        assert_eq!(cleaned, "William");
        assert_eq!(nicks, vec!["Ken"]);
    }

    #[test]
    fn extract_quoted_nickname() {
        let (cleaned, nicks) = extract_nicknames("Thomas \"Butch\"");
        assert_eq!(cleaned, "Thomas");
        assert_eq!(nicks, vec!["Butch"]);
    }

    #[test]
    fn extract_paren_with_extra_text() {
        let (cleaned, nicks) = extract_nicknames("John (jack) C.");
        assert_eq!(cleaned, "John C.");
        assert_eq!(nicks, vec!["jack"]);
    }

    #[test]
    fn extract_no_nicknames() {
        let (cleaned, nicks) = extract_nicknames("Maria Elena");
        assert_eq!(cleaned, "Maria Elena");
        assert!(nicks.is_empty());
    }

    // -----------------------------------------------------------------------
    // Suffix extraction
    // -----------------------------------------------------------------------

    #[test]
    fn extract_suffix_iii() {
        let (name, suffix) = extract_suffix("LeBlanc III");
        assert_eq!(name, "LeBlanc");
        assert_eq!(suffix, Some("III".to_string()));
    }

    #[test]
    fn extract_suffix_jr_period() {
        let (name, suffix) = extract_suffix("Smith Jr.");
        assert_eq!(name, "Smith");
        assert_eq!(suffix, Some("Jr.".to_string()));
    }

    #[test]
    fn extract_no_suffix() {
        let (name, suffix) = extract_suffix("García");
        assert_eq!(name, "García");
        assert_eq!(suffix, None);
    }

    // -----------------------------------------------------------------------
    // Junk stripping
    // -----------------------------------------------------------------------

    #[test]
    fn strip_trailing_comma() {
        assert_eq!(strip_junk("Cronenberger,"), "Cronenberger");
    }

    #[test]
    fn strip_email_address() {
        assert_eq!(strip_junk("Neel.Baumgardner@utsa.edu"), "");
    }

    #[test]
    fn strip_clean_name() {
        assert_eq!(strip_junk("  Maria  "), "Maria");
    }

    // -----------------------------------------------------------------------
    // normalize_for_matching
    // -----------------------------------------------------------------------

    #[test]
    fn normalize_strips_accents() {
        assert_eq!(normalize_for_matching("García"), "garcia");
    }

    #[test]
    fn normalize_strips_apostrophe() {
        assert_eq!(normalize_for_matching("O'Brien"), "obrien");
    }

    #[test]
    fn normalize_strips_hyphen() {
        assert_eq!(normalize_for_matching("Aguirre-Mesa"), "aguirremesa");
    }

    #[test]
    fn normalize_tilde_n() {
        assert_eq!(normalize_for_matching("Muñoz"), "munoz");
    }

    #[test]
    fn normalize_umlaut() {
        assert_eq!(normalize_for_matching("Müller"), "muller");
    }

    #[test]
    fn normalize_period() {
        assert_eq!(normalize_for_matching("H. Paul"), "hpaul");
    }

    #[test]
    fn normalize_strips_spaces() {
        assert_eq!(normalize_for_matching("Mary Lou"), "marylou");
    }

    // -----------------------------------------------------------------------
    // parse_banner_name
    // -----------------------------------------------------------------------

    #[test]
    fn banner_standard_name() {
        let p = parse_banner_name("Smith, John").unwrap();
        assert_eq!(p.first, "John");
        assert_eq!(p.last, "Smith");
        assert_eq!(p.suffix, None);
    }

    #[test]
    fn banner_html_entity_apostrophe() {
        let p = parse_banner_name("O&#39;Brien, Erin").unwrap();
        assert_eq!(p.first, "Erin");
        assert_eq!(p.last, "O'Brien");
    }

    #[test]
    fn banner_html_entity_umlaut() {
        let p = parse_banner_name("Temel, B&uuml;lent").unwrap();
        assert_eq!(p.first, "Bülent");
        assert_eq!(p.last, "Temel");
    }

    #[test]
    fn banner_suffix_iii() {
        let p = parse_banner_name("LeBlanc III, H. Paul").unwrap();
        assert_eq!(p.first, "H. Paul");
        assert_eq!(p.last, "LeBlanc");
        assert_eq!(p.suffix, Some("III".to_string()));
    }

    #[test]
    fn banner_suffix_ii() {
        let p = parse_banner_name("Ellis II, Ronald").unwrap();
        assert_eq!(p.first, "Ronald");
        assert_eq!(p.last, "Ellis");
        assert_eq!(p.suffix, Some("II".to_string()));
    }

    #[test]
    fn banner_multi_word_last() {
        let p = parse_banner_name("Aguirre Mesa, Andres").unwrap();
        assert_eq!(p.first, "Andres");
        assert_eq!(p.last, "Aguirre Mesa");
    }

    #[test]
    fn banner_hyphenated_last() {
        let p = parse_banner_name("Abu-Lail, Nehal").unwrap();
        assert_eq!(p.first, "Nehal");
        assert_eq!(p.last, "Abu-Lail");
    }

    #[test]
    fn banner_with_middle_name() {
        let p = parse_banner_name("Smith, John David").unwrap();
        assert_eq!(p.first, "John David");
        assert_eq!(p.last, "Smith");
    }

    #[test]
    fn banner_no_comma() {
        assert!(parse_banner_name("SingleName").is_none());
    }

    #[test]
    fn banner_empty_first() {
        assert!(parse_banner_name("Smith,").is_none());
    }

    #[test]
    fn banner_empty_last() {
        assert!(parse_banner_name(", John").is_none());
    }

    // -----------------------------------------------------------------------
    // parse_rmp_name
    // -----------------------------------------------------------------------

    #[test]
    fn rmp_standard_name() {
        let p = parse_rmp_name("John", "Smith").unwrap();
        assert_eq!(p.first, "John");
        assert_eq!(p.last, "Smith");
    }

    #[test]
    fn rmp_with_nickname() {
        let p = parse_rmp_name("William (Ken)", "Burchenal").unwrap();
        assert_eq!(p.first, "William");
        assert_eq!(p.nicknames, vec!["Ken"]);
    }

    #[test]
    fn rmp_trailing_comma_last() {
        let p = parse_rmp_name("J.", "Cronenberger,").unwrap();
        assert_eq!(p.last, "Cronenberger");
    }

    #[test]
    fn rmp_email_in_first() {
        assert!(parse_rmp_name("Neel.Baumgardner@utsa.edu", "Baumgardner").is_none());
    }

    #[test]
    fn rmp_suffix_in_last() {
        let p = parse_rmp_name("H. Paul", "LeBlanc III").unwrap();
        assert_eq!(p.first, "H. Paul");
        assert_eq!(p.last, "LeBlanc");
        assert_eq!(p.suffix, Some("III".to_string()));
    }

    #[test]
    fn rmp_quoted_nickname() {
        let p = parse_rmp_name("Thomas \"Butch\"", "Matjeka").unwrap();
        assert_eq!(p.first, "Thomas");
        assert_eq!(p.nicknames, vec!["Butch"]);
    }

    #[test]
    fn rmp_accented_last() {
        let p = parse_rmp_name("Liliana", "Saldaña").unwrap();
        assert_eq!(p.last, "Saldaña");
    }

    // -----------------------------------------------------------------------
    // matching_keys
    // -----------------------------------------------------------------------

    #[test]
    fn keys_simple_name() {
        let parts = NameParts {
            first: "John".into(),
            last: "Smith".into(),
            middle: None,
            suffix: None,
            nicknames: vec![],
        };
        let keys = matching_keys(&parts);
        assert_eq!(keys, vec![("smith".into(), "john".into())]);
    }

    #[test]
    fn keys_multi_token_first() {
        let parts = NameParts {
            first: "H. Paul".into(),
            last: "LeBlanc".into(),
            middle: None,
            suffix: Some("III".into()),
            nicknames: vec![],
        };
        let keys = matching_keys(&parts);
        assert!(keys.contains(&("leblanc".into(), "hpaul".into())));
        assert!(keys.contains(&("leblanc".into(), "paul".into())));
        assert!(keys.contains(&("leblanc".into(), "h".into())));
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn keys_with_nickname() {
        let parts = NameParts {
            first: "William".into(),
            last: "Burchenal".into(),
            middle: None,
            suffix: None,
            nicknames: vec!["Ken".into()],
        };
        let keys = matching_keys(&parts);
        assert!(keys.contains(&("burchenal".into(), "william".into())));
        assert!(keys.contains(&("burchenal".into(), "ken".into())));
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn keys_hyphenated_last() {
        let parts = parse_banner_name("Aguirre-Mesa, Andres").unwrap();
        let keys = matching_keys(&parts);
        // Hyphen removed: "aguirremesa"
        assert!(keys.contains(&("aguirremesa".into(), "andres".into())));
    }

    #[test]
    fn keys_accented_name() {
        let parts = parse_rmp_name("Liliana", "Saldaña").unwrap();
        let keys = matching_keys(&parts);
        assert!(keys.contains(&("saldana".into(), "liliana".into())));
    }

    #[test]
    fn keys_cross_source_match() {
        // Banner: "Aguirre Mesa, Andres" → last="Aguirre Mesa"
        let banner = parse_banner_name("Aguirre Mesa, Andres").unwrap();
        let banner_keys = matching_keys(&banner);

        // RMP: "Andres" / "Aguirre-Mesa" → last="Aguirre-Mesa"
        let rmp = parse_rmp_name("Andres", "Aguirre-Mesa").unwrap();
        let rmp_keys = matching_keys(&rmp);

        // Both should normalize to ("aguirremesa", "andres")
        assert!(banner_keys.iter().any(|k| rmp_keys.contains(k)));
    }

    #[test]
    fn keys_accent_cross_match() {
        // Banner: "García, José" (if Banner ever has accents)
        let banner = parse_banner_name("Garcia, Jose").unwrap();
        let banner_keys = matching_keys(&banner);

        // RMP: "José" / "García"
        let rmp = parse_rmp_name("José", "García").unwrap();
        let rmp_keys = matching_keys(&rmp);

        // Both normalize to ("garcia", "jose")
        assert!(banner_keys.iter().any(|k| rmp_keys.contains(k)));
    }
}
