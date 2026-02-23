use std::{ops::RangeInclusive, str::FromStr};

use anyhow::Context;
use chrono::{Datelike, Local, NaiveDate};
use serde::{Deserialize, Serialize};

/// The current year at the time of compilation
const CURRENT_YEAR: u32 = compile_time::date!().year() as u32;

/// The valid years for terms.
/// Lower bound is 2001 — the earliest term UTSA's Banner system serves is Fall 2001 (200110).
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Season {
    Fall,
    Spring,
    Summer,
}

impl Term {
    /// Returns the current term status - either currently in a term or between terms
    pub fn get_current() -> TermPoint {
        let now = Local::now().naive_local();
        Self::get_status_for_date(now.date())
    }

    /// Returns the current term status for a specific date
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
                next: Term {
                    year: term_year,
                    season: Season::Spring,
                },
            }
        } else if (day_of_year >= ranges.spring.start) && (day_of_year < ranges.spring.end) {
            // Spring
            TermPoint::InTerm {
                current: Term {
                    year: term_year,
                    season: Season::Spring,
                },
            }
        } else if day_of_year < ranges.summer.start {
            // Spring over, Summer not yet begun
            TermPoint::BetweenTerms {
                next: Term {
                    year: term_year,
                    season: Season::Summer,
                },
            }
        } else if (day_of_year >= ranges.summer.start) && (day_of_year < ranges.summer.end) {
            // Summer
            TermPoint::InTerm {
                current: Term {
                    year: term_year,
                    season: Season::Summer,
                },
            }
        } else if day_of_year < ranges.fall.start {
            // Summer over, Fall not yet begun
            TermPoint::BetweenTerms {
                next: Term {
                    year: term_year,
                    season: Season::Fall,
                },
            }
        } else if (day_of_year >= ranges.fall.start) && (day_of_year < ranges.fall.end) {
            // Fall
            TermPoint::InTerm {
                current: Term {
                    year: term_year,
                    season: Season::Fall,
                },
            }
        } else {
            // This should never happen, but Rust requires exhaustive matching
            panic!("Impossible code reached (dayOfYear: {})", day_of_year);
        }
    }

    /// Returns the start and end day of each term for the given year.
    /// The ranges are inclusive of the start day and exclusive of the end day.
    fn get_season_ranges(year: u32) -> SeasonRanges {
        let spring_start = NaiveDate::from_ymd_opt(year as i32, 1, 14)
            .unwrap()
            .ordinal();
        let spring_end = NaiveDate::from_ymd_opt(year as i32, 5, 1)
            .unwrap()
            .ordinal();
        let summer_start = NaiveDate::from_ymd_opt(year as i32, 5, 25)
            .unwrap()
            .ordinal();
        let summer_end = NaiveDate::from_ymd_opt(year as i32, 8, 15)
            .unwrap()
            .ordinal();
        let fall_start = NaiveDate::from_ymd_opt(year as i32, 8, 18)
            .unwrap()
            .ordinal();
        let fall_end = NaiveDate::from_ymd_opt(year as i32, 12, 10)
            .unwrap()
            .ordinal();

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
    pub fn slug(&self) -> String {
        format!("{}-{}", self.season.slug(), self.year)
    }

    /// Parse a slug like "spring-2026" into a Term
    pub fn from_slug(s: &str) -> Option<Self> {
        let (season_str, year_str) = s.rsplit_once('-')?;
        let season = Season::from_slug(season_str)?;
        let year = year_str.parse::<u32>().ok()?;
        if !VALID_YEARS.contains(&year) {
            return None;
        }
        Some(Term { year, season })
    }

    /// Human-readable description, e.g. "Spring 2026"
    pub fn description(&self) -> String {
        format!("{} {}", self.season, self.year)
    }

    /// Resolve a string that is either a term code ("202620") or a slug ("spring-2026") to a term code.
    pub fn resolve_to_code(s: &str) -> Option<String> {
        // Try parsing as a 6-digit code first
        if let Ok(term) = s.parse::<Term>() {
            return Some(term.to_string());
        }
        // Try parsing as a slug
        Term::from_slug(s).map(|t| t.to_string())
    }
}

impl TermPoint {
    /// Returns the inner Term regardless of the status
    pub fn inner(&self) -> &Term {
        match self {
            TermPoint::InTerm { current } => current,
            TermPoint::BetweenTerms { next } => next,
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
    /// Returns the term in the format YYYYXX, where YYYY is the year and XX is the season code
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{year}{season}",
            year = self.year,
            season = self.season.to_str()
        )
    }
}

impl Season {
    /// Returns the season code as a string
    fn to_str(self) -> &'static str {
        match self {
            Season::Fall => "10",
            Season::Spring => "20",
            Season::Summer => "30",
        }
    }

    /// Returns the lowercase slug for URL-friendly representation
    pub fn slug(self) -> &'static str {
        match self {
            Season::Fall => "fall",
            Season::Spring => "spring",
            Season::Summer => "summer",
        }
    }

    /// Parse a slug like "spring", "summer", "fall" into a Season
    pub fn from_slug(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "fall" => Some(Season::Fall),
            "spring" => Some(Season::Spring),
            "summer" => Some(Season::Summer),
            _ => None,
        }
    }
}

impl std::fmt::Display for Season {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Season::Fall => write!(f, "Fall"),
            Season::Spring => write!(f, "Spring"),
            Season::Summer => write!(f, "Summer"),
        }
    }
}

impl FromStr for Season {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let season = match s {
            "10" => Season::Fall,
            "20" => Season::Spring,
            "30" => Season::Summer,
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

        let year = s[0..4].parse::<u32>().context("Failed to parse year")?;
        if !VALID_YEARS.contains(&year) {
            return Err(anyhow::anyhow!("Year out of range"));
        }

        let season =
            Season::from_str(&s[4..6]).map_err(|e| anyhow::anyhow!("Invalid season: {}", e))?;

        Ok(Term { year, season })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Season::from_str ---

    #[test]
    fn test_season_from_str_fall() {
        assert_eq!(Season::from_str("10").unwrap(), Season::Fall);
    }

    #[test]
    fn test_season_from_str_spring() {
        assert_eq!(Season::from_str("20").unwrap(), Season::Spring);
    }

    #[test]
    fn test_season_from_str_summer() {
        assert_eq!(Season::from_str("30").unwrap(), Season::Summer);
    }

    #[test]
    fn test_season_from_str_invalid() {
        for input in ["00", "40", "1", ""] {
            assert!(
                Season::from_str(input).is_err(),
                "expected Err for {input:?}"
            );
        }
    }

    // --- Season Display ---

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

    // --- Term::from_str ---

    #[test]
    fn test_term_from_str_valid_fall() {
        let term = Term::from_str("202510").unwrap();
        assert_eq!(term.year, 2025);
        assert_eq!(term.season, Season::Fall);
    }

    #[test]
    fn test_term_from_str_valid_spring() {
        let term = Term::from_str("202520").unwrap();
        assert_eq!(term.year, 2025);
        assert_eq!(term.season, Season::Spring);
    }

    #[test]
    fn test_term_from_str_valid_summer() {
        let term = Term::from_str("202530").unwrap();
        assert_eq!(term.year, 2025);
        assert_eq!(term.season, Season::Summer);
    }

    #[test]
    fn test_term_from_str_too_short() {
        assert!(Term::from_str("20251").is_err());
    }

    #[test]
    fn test_term_from_str_too_long() {
        assert!(Term::from_str("2025100").is_err());
    }

    #[test]
    fn test_term_from_str_empty() {
        assert!(Term::from_str("").is_err());
    }

    #[test]
    fn test_term_from_str_invalid_year_chars() {
        assert!(Term::from_str("abcd10").is_err());
    }

    #[test]
    fn test_term_from_str_invalid_season() {
        assert!(Term::from_str("202540").is_err());
    }

    #[test]
    fn test_term_from_str_year_below_range() {
        assert!(Term::from_str("200010").is_err());
    }

    #[test]
    fn test_term_display_roundtrip() {
        for code in ["202510", "202520", "202530"] {
            let term = Term::from_str(code).unwrap();
            assert_eq!(term.to_string(), code);
        }
    }

    // --- Term::get_status_for_date ---

    #[test]
    fn test_status_mid_spring() {
        let date = NaiveDate::from_ymd_opt(2025, 2, 15).unwrap();
        let status = Term::get_status_for_date(date);
        assert!(
            matches!(status, TermPoint::InTerm { current } if current.season == Season::Spring)
        );
    }

    #[test]
    fn test_status_mid_summer() {
        let date = NaiveDate::from_ymd_opt(2025, 7, 1).unwrap();
        let status = Term::get_status_for_date(date);
        assert!(
            matches!(status, TermPoint::InTerm { current } if current.season == Season::Summer)
        );
    }

    #[test]
    fn test_status_mid_fall() {
        let date = NaiveDate::from_ymd_opt(2025, 10, 15).unwrap();
        let status = Term::get_status_for_date(date);
        assert!(matches!(status, TermPoint::InTerm { current } if current.season == Season::Fall));
    }

    #[test]
    fn test_status_between_fall_and_spring() {
        let date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let status = Term::get_status_for_date(date);
        assert!(
            matches!(status, TermPoint::BetweenTerms { next } if next.season == Season::Spring)
        );
    }

    #[test]
    fn test_status_between_spring_and_summer() {
        let date = NaiveDate::from_ymd_opt(2025, 5, 15).unwrap();
        let status = Term::get_status_for_date(date);
        assert!(
            matches!(status, TermPoint::BetweenTerms { next } if next.season == Season::Summer)
        );
    }

    #[test]
    fn test_status_between_summer_and_fall() {
        let date = NaiveDate::from_ymd_opt(2025, 8, 16).unwrap();
        let status = Term::get_status_for_date(date);
        assert!(matches!(status, TermPoint::BetweenTerms { next } if next.season == Season::Fall));
    }

    #[test]
    fn test_status_after_fall_end() {
        let date = NaiveDate::from_ymd_opt(2025, 12, 15).unwrap();
        let status = Term::get_status_for_date(date);
        assert!(
            matches!(status, TermPoint::BetweenTerms { next } if next.season == Season::Spring)
        );
        // Year should roll over: fall 2025 ends → next spring is 2026
        let next_term = status.inner();
        assert_eq!(next_term.year, 2026);
    }

    // --- TermPoint::inner ---

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

    // --- Season::slug / from_slug ---

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
        assert_eq!(
            Term::resolve_to_code("Spring-2026"),
            Some("202620".to_string())
        );
    }

    // --- Term::slug / from_slug ---

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

    // --- Term::description ---

    #[test]
    fn test_term_description() {
        let term = Term {
            year: 2026,
            season: Season::Spring,
        };
        assert_eq!(term.description(), "Spring 2026");
    }

    // --- Term::resolve_to_code ---

    #[test]
    fn test_resolve_to_code_from_code() {
        assert_eq!(Term::resolve_to_code("202620"), Some("202620".to_string()));
    }

    #[test]
    fn test_resolve_to_code_from_slug() {
        assert_eq!(
            Term::resolve_to_code("spring-2026"),
            Some("202620".to_string())
        );
    }

    #[test]
    fn test_resolve_to_code_invalid() {
        assert_eq!(Term::resolve_to_code("garbage"), None);
    }
}
