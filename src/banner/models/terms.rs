use std::{ops::RangeInclusive, str::FromStr};

use anyhow::Context;
use chrono::{Datelike, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, VariantArray};
use ts_rs::TS;

use crate::data::models::UnknownVariant;

/// The current year at the time of compilation
#[expect(clippy::cast_sign_loss, reason = "compile-time calendar year is always positive")]
const CURRENT_YEAR: u32 = compile_time::date!().year() as u32;

/// The valid years for terms, in **display-year** terms (not Banner code prefix).
///
/// Banner encodes Fall terms as `(display_year + 1)10`: Fall 2001 -> code `200210`.
/// This range validates the human-readable year, so Fall 2001 (`display_year` 2001) is accepted.
///
/// Lower bound is 2001: the earliest term UTSA's Banner system serves is Fall 2001 (200210).
/// Upper bound is compile-time year + 10 to stay tight without manual updates.
const VALID_YEARS: RangeInclusive<u32> = 2001..=(CURRENT_YEAR + 10);

/// Represents a term in the Banner system
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Term {
    pub year: u32, // 2024, 2025, etc
    pub season: Season,
}

/// Represents the term status at a specific point in time
#[derive(Debug, Clone)]
pub enum TermPoint {
    /// Currently in a term
    InTerm { current: Term },
    /// Between terms, with the next term specified
    BetweenTerms { next: Term },
}

/// Represents a season within a term
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, TS, AsRefStr, VariantArray)]
#[ts(export)]
pub enum Season {
    Fall,
    Spring,
    Summer,
}

impl Term {
    /// Returns the current term status - either currently in a term or between terms
    #[must_use]
    pub fn get_current() -> TermPoint {
        let now = Local::now().naive_local();
        Self::get_status_for_date(now.date())
    }

    /// Returns the current term status for a specific date
    ///
    /// # Panics
    ///
    /// Never panics in practice: the season branches below are exhaustive for any
    /// day of the year, so the final `panic!` is unreachable.
    #[must_use]
    #[expect(
        clippy::cast_sign_loss,
        reason = "callers pass present-day calendar dates, so year() is positive"
    )]
    pub fn get_status_for_date(date: NaiveDate) -> TermPoint {
        let literal_year = date.year() as u32;
        let day_of_year = date.ordinal();
        let ranges = Self::get_season_ranges(literal_year);

        // If we're past the end of the summer term, we're 'in' the next school year.
        let term_year = if day_of_year > ranges.summer.end {
            literal_year + 1
        } else {
            literal_year
        };

        if (day_of_year < ranges.spring.start) || (day_of_year >= ranges.fall.end) {
            // Fall over, Spring not yet begun
            TermPoint::BetweenTerms {
                next: Self {
                    year: term_year,
                    season: Season::Spring,
                },
            }
        } else if (day_of_year >= ranges.spring.start) && (day_of_year < ranges.spring.end) {
            // Spring
            TermPoint::InTerm {
                current: Self {
                    year: term_year,
                    season: Season::Spring,
                },
            }
        } else if day_of_year < ranges.summer.start {
            // Spring over, Summer not yet begun
            TermPoint::BetweenTerms {
                next: Self {
                    year: term_year,
                    season: Season::Summer,
                },
            }
        } else if (day_of_year >= ranges.summer.start) && (day_of_year < ranges.summer.end) {
            // Summer
            TermPoint::InTerm {
                current: Self {
                    year: term_year,
                    season: Season::Summer,
                },
            }
        } else if day_of_year < ranges.fall.start {
            // Summer over, Fall not yet begun.
            // Fall display year matches the literal calendar year (e.g., Fall 2025 runs in 2025).
            TermPoint::BetweenTerms {
                next: Self {
                    year: literal_year,
                    season: Season::Fall,
                },
            }
        } else if (day_of_year >= ranges.fall.start) && (day_of_year < ranges.fall.end) {
            // Fall. Display year matches the literal calendar year.
            TermPoint::InTerm {
                current: Self {
                    year: literal_year,
                    season: Season::Fall,
                },
            }
        } else {
            // This should never happen, but Rust requires exhaustive matching
            panic!("Impossible code reached (dayOfYear: {day_of_year})");
        }
    }

    /// Returns the start and end day of each term for the given year.
    /// The ranges are inclusive of the start day and exclusive of the end day.
    #[expect(clippy::cast_possible_wrap, reason = "year is a calendar year, far under i32::MAX")]
    fn get_season_ranges(year: u32) -> SeasonRanges {
        let spring_start = NaiveDate::from_ymd_opt(year as i32, 1, 14).unwrap().ordinal();
        let spring_end = NaiveDate::from_ymd_opt(year as i32, 5, 1).unwrap().ordinal();
        let summer_start = NaiveDate::from_ymd_opt(year as i32, 5, 25).unwrap().ordinal();
        let summer_end = NaiveDate::from_ymd_opt(year as i32, 8, 15).unwrap().ordinal();
        let fall_start = NaiveDate::from_ymd_opt(year as i32, 8, 18).unwrap().ordinal();
        let fall_end = NaiveDate::from_ymd_opt(year as i32, 12, 10).unwrap().ordinal();

        SeasonRanges {
            spring: YearDayRange {
                start: spring_start,
                end: spring_end,
            },
            summer: YearDayRange {
                start: summer_start,
                end: summer_end,
            },
            fall: YearDayRange {
                start: fall_start,
                end: fall_end,
            },
        }
    }

    /// URL-friendly slug, e.g. "spring-2026"
    #[must_use]
    pub fn slug(self) -> String {
        format!("{}-{}", self.season.slug(), self.year)
    }

    /// Parse a slug like "spring-2026" into a Term
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        let (season_str, year_str) = s.rsplit_once('-')?;
        let season = Season::from_slug(season_str)?;
        let year = year_str.parse::<u32>().ok()?;
        if !VALID_YEARS.contains(&year) {
            return None;
        }
        Some(Self { year, season })
    }

    /// Human-readable description, e.g. "Spring 2026"
    #[must_use]
    pub fn description(self) -> String {
        format!("{} {}", self.season, self.year)
    }

    /// Resolve a string that is either a term code ("202620") or a slug ("spring-2026") to a term code.
    #[must_use]
    pub fn resolve_to_code(s: &str) -> Option<String> {
        // Try parsing as a 6-digit code first
        if let Ok(term) = s.parse::<Self>() {
            return Some(term.to_string());
        }
        // Try parsing as a slug
        Self::from_slug(s).map(|t| t.to_string())
    }
}

impl TermPoint {
    /// Returns the inner Term regardless of the status
    #[must_use]
    pub const fn inner(&self) -> &Term {
        match self {
            Self::InTerm { current } => current,
            Self::BetweenTerms { next } => next,
        }
    }
}

/// Represents the start and end day of each term within a year
#[derive(Debug, Clone)]
struct SeasonRanges {
    spring: YearDayRange,
    summer: YearDayRange,
    fall: YearDayRange,
}

/// Represents the start and end day of a term within a year
#[derive(Debug, Clone)]
struct YearDayRange {
    start: u32,
    end: u32,
}

impl std::fmt::Display for Term {
    /// Returns the Banner term code in the format YYYYXX.
    ///
    /// Banner encodes Fall terms as `(display_year + 1)10`, so `Term { year: 2025, season: Fall }`
    /// produces `"202610"`: the prefix is `2025 + 1 = 2026`. Spring and Summer use the
    /// display year directly as the prefix.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code_year = match self.season {
            Season::Fall => self.year + 1,
            _ => self.year,
        };
        write!(f, "{code_year}{season}", season = self.season.to_str())
    }
}

impl Season {
    /// Returns the season code as a string
    const fn to_str(self) -> &'static str {
        match self {
            Self::Fall => "10",
            Self::Spring => "20",
            Self::Summer => "30",
        }
    }

    /// Returns the lowercase slug for URL-friendly representation
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Fall => "fall",
            Self::Spring => "spring",
            Self::Summer => "summer",
        }
    }

    /// Parse a slug like "spring", "summer", "fall" into a Season
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "fall" => Some(Self::Fall),
            "spring" => Some(Self::Spring),
            "summer" => Some(Self::Summer),
            _ => None,
        }
    }
}

/// Codec for the `terms.season` column, which stores the variant name, not the code.
///
/// `FromStr` is already taken by the two-digit season code, so this cannot use `EnumString`.
impl sqlx::Type<sqlx::Postgres> for Season {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <str as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <&str as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for Season {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let name = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Self::from_slug(name).ok_or_else(|| UnknownVariant::new("Season", name).into())
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for Season {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <&str as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&self.as_ref(), buf)
    }
}

impl std::fmt::Display for Season {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl FromStr for Season {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let season = match s {
            "10" => Self::Fall,
            "20" => Self::Spring,
            "30" => Self::Summer,
            _ => return Err(anyhow::anyhow!("Invalid season: {s}")),
        };
        Ok(season)
    }
}

impl FromStr for Term {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 6 {
            return Err(anyhow::anyhow!("Term string must be 6 characters"));
        }

        let code_year = s[0..4].parse::<u32>().context("Failed to parse year")?;
        let season = Season::from_str(&s[4..6]).map_err(|e| anyhow::anyhow!("Invalid season: {e}"))?;

        // Banner encodes Fall as (display_year + 1)10, so we subtract 1 to get the year
        // that matches the human-readable description (e.g., "200210" -> Fall 2001).
        let year = match season {
            Season::Fall => code_year - 1,
            _ => code_year,
        };

        if !VALID_YEARS.contains(&year) {
            return Err(anyhow::anyhow!("Year out of range"));
        }

        Ok(Self { year, season })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::fall("10", Season::Fall)]
    #[case::spring("20", Season::Spring)]
    #[case::summer("30", Season::Summer)]
    fn test_season_from_str_valid(#[case] input: &str, #[case] expected: Season) {
        assert_eq!(Season::from_str(input).unwrap(), expected);
    }

    #[test]
    fn test_season_from_str_invalid() {
        for input in ["00", "40", "1", ""] {
            assert!(Season::from_str(input).is_err(), "expected Err for {input:?}");
        }
    }

    #[test]
    fn test_season_display() {
        assert_eq!(Season::Fall.to_string(), "Fall");
        assert_eq!(Season::Spring.to_string(), "Spring");
        assert_eq!(Season::Summer.to_string(), "Summer");
    }

    #[test]
    fn test_season_to_str_roundtrip() {
        for season in [Season::Fall, Season::Spring, Season::Summer] {
            assert_eq!(Season::from_str(season.to_str()).unwrap(), season);
        }
    }

    #[rstest]
    #[case::fall("202510", 2024, Season::Fall)]
    #[case::spring("202520", 2025, Season::Spring)]
    #[case::summer("202530", 2025, Season::Summer)]
    fn test_term_from_str_valid(#[case] code: &str, #[case] expected_year: u32, #[case] expected_season: Season) {
        // "202510": code_year=2025, Fall -> display_year = 2025 - 1 = 2024
        let term = Term::from_str(code).unwrap();
        assert_eq!(term.year, expected_year);
        assert_eq!(term.season, expected_season);
    }

    #[test]
    fn test_term_from_str_fall_earliest() {
        // Fall 2001 is the earliest UTSA term, with Banner code "200210"
        let term = Term::from_str("200210").unwrap();
        assert_eq!(term.year, 2001);
        assert_eq!(term.season, Season::Fall);
    }

    #[test]
    fn test_term_from_str_fall_display_year() {
        // "202610" -> Fall 2025: code prefix is 2026 (display_year + 1)
        let term = Term::from_str("202610").unwrap();
        assert_eq!(term.year, 2025);
        assert_eq!(term.season, Season::Fall);
        assert_eq!(term.description(), "Fall 2025");
        assert_eq!(term.slug(), "fall-2025");
        assert_eq!(term.to_string(), "202610");
    }

    #[rstest]
    #[case::too_short("20251")]
    #[case::too_long("2025100")]
    #[case::empty("")]
    #[case::invalid_year_chars("abcd10")]
    #[case::invalid_season("202540")]
    fn test_term_from_str_invalid(#[case] input: &str) {
        assert!(Term::from_str(input).is_err());
    }

    #[test]
    fn test_term_from_str_year_below_range() {
        // "200010": code_year=2000, Fall -> display_year=1999, below VALID_YEARS lower bound
        assert!(Term::from_str("200010").is_err());
        // "200110": code_year=2001, Fall -> display_year=2000, still below lower bound
        assert!(Term::from_str("200110").is_err());
    }

    #[test]
    fn test_term_display_roundtrip() {
        for code in ["202510", "202520", "202530"] {
            let term = Term::from_str(code).unwrap();
            assert_eq!(term.to_string(), code);
        }
    }

    #[rstest]
    #[case::mid_spring((2025, 2, 15), true, Season::Spring, None)]
    #[case::mid_summer((2025, 7, 1), true, Season::Summer, None)]
    #[case::mid_fall((2025, 10, 15), true, Season::Fall, None)]
    #[case::between_fall_and_spring((2025, 1, 1), false, Season::Spring, None)]
    #[case::between_spring_and_summer((2025, 5, 15), false, Season::Summer, None)]
    #[case::between_summer_and_fall((2025, 8, 16), false, Season::Fall, None)]
    // Year should roll over: fall 2025 ends -> next spring is 2026.
    #[case::after_fall_end((2025, 12, 15), false, Season::Spring, Some(2026))]
    fn test_status(
        #[case] ymd: (i32, u32, u32),
        #[case] in_term: bool,
        #[case] expected_season: Season,
        #[case] expected_year: Option<u32>,
    ) {
        let (y, m, d) = ymd;
        let date = NaiveDate::from_ymd_opt(y, m, d).unwrap();
        let status = Term::get_status_for_date(date);
        match (&status, in_term) {
            (TermPoint::InTerm { current }, true) => assert_eq!(current.season, expected_season),
            (TermPoint::BetweenTerms { next }, false) => assert_eq!(next.season, expected_season),
            _ => panic!("unexpected term point variant: {status:?}"),
        }
        if let Some(year) = expected_year {
            assert_eq!(status.inner().year, year);
        }
    }

    #[test]
    fn test_term_point_inner() {
        let in_term = TermPoint::InTerm {
            current: Term {
                year: 2025,
                season: Season::Fall,
            },
        };
        assert_eq!(
            in_term.inner(),
            &Term {
                year: 2025,
                season: Season::Fall
            }
        );

        let between = TermPoint::BetweenTerms {
            next: Term {
                year: 2026,
                season: Season::Spring,
            },
        };
        assert_eq!(
            between.inner(),
            &Term {
                year: 2026,
                season: Season::Spring
            }
        );
    }

    #[test]
    fn test_season_slug_roundtrip() {
        for season in [Season::Fall, Season::Spring, Season::Summer] {
            assert_eq!(Season::from_slug(season.slug()), Some(season));
        }
    }

    #[test]
    fn test_season_from_slug_invalid() {
        assert_eq!(Season::from_slug("winter"), None);
        assert_eq!(Season::from_slug(""), None);
    }

    #[test]
    fn test_season_from_slug_case_insensitive() {
        assert_eq!(Season::from_slug("Spring"), Some(Season::Spring));
        assert_eq!(Season::from_slug("FALL"), Some(Season::Fall));
        assert_eq!(Season::from_slug("Summer"), Some(Season::Summer));
    }

    #[test]
    fn test_term_from_slug_case_insensitive() {
        let term = Term::from_slug("Spring-2026").unwrap();
        assert_eq!(term.year, 2026);
        assert_eq!(term.season, Season::Spring);
    }

    #[test]
    fn test_resolve_to_code_case_insensitive() {
        assert_eq!(Term::resolve_to_code("Spring-2026"), Some("202620".to_string()));
    }

    #[test]
    fn test_term_slug() {
        let term = Term {
            year: 2026,
            season: Season::Spring,
        };
        assert_eq!(term.slug(), "spring-2026");
    }

    #[test]
    fn test_term_from_slug_roundtrip() {
        for code in ["202510", "202520", "202530"] {
            let term = Term::from_str(code).unwrap();
            let slug = term.slug();
            let parsed = Term::from_slug(&slug).unwrap();
            assert_eq!(parsed, term);
        }
    }

    #[test]
    fn test_term_from_slug_invalid() {
        assert_eq!(Term::from_slug("winter-2026"), None);
        assert_eq!(Term::from_slug("spring"), None);
        assert_eq!(Term::from_slug(""), None);
    }

    #[test]
    fn test_term_description() {
        let term = Term {
            year: 2026,
            season: Season::Spring,
        };
        assert_eq!(term.description(), "Spring 2026");
    }

    #[rstest]
    #[case::from_code("202620", Some("202620"))]
    #[case::from_slug("spring-2026", Some("202620"))]
    #[case::invalid("garbage", None)]
    fn test_resolve_to_code(#[case] input: &str, #[case] expected: Option<&str>) {
        assert_eq!(
            Term::resolve_to_code(input),
            expected.map(std::string::ToString::to_string)
        );
    }
}
