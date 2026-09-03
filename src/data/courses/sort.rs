//! Sort keys and query specs for course search ordering.
//!
//! Defines the orderable `SortKey`s, how each renders to SQL, and the `SortSpec` that parses and composes them from the wire format.

use std::fmt;
use std::str::FromStr;
use strum::VariantArray;
use ts_rs::TS;

/// An orderable quantity.
///
/// Deliberately not a column: several keys may share one column's header, and
/// some belong to no column at all.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize, TS, VariantArray,
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SortKey {
    CourseCode,
    Title,
    InstructorName,
    InstructorRating,
    StartTime,
    EndTime,
    Duration,
    WeeklyMinutes,
    Days,
    SeatsOpen,
    FillRatio,
    WaitCount,
}

/// How a key orders, and what each direction means to a reader.
///
/// Labels live beside the expression so a key cannot gain an ordering without
/// also saying what that ordering means. "Ascending" never reaches the UI.
struct KeyDef {
    /// Wire name. Must match the serde representation; a test holds them together.
    name: &'static str,
    /// ORDER BY columns, each taking the term's direction. A key spanning several
    /// columns must list them separately or only the last one would reverse.
    /// Hardcoded literals, never built from caller input.
    exprs: &'static [&'static str],
    asc: &'static str,
    desc: &'static str,
}

impl SortKey {
    /// Every variant, derived by strum so a new one cannot be left unreachable.
    pub const ALL: &'static [SortKey] = Self::VARIANTS;

    const fn def(self) -> KeyDef {
        match self {
            Self::CourseCode => KeyDef {
                name: "course_code",
                exprs: &["subject", "course_number", "sequence_number"],
                asc: "A to Z",
                desc: "Z to A",
            },
            Self::Title => KeyDef {
                name: "title",
                exprs: &["title"],
                asc: "A to Z",
                desc: "Z to A",
            },
            // display_name is stored "Last, First", so this orders by last name
            // then first without needing to split it.
            Self::InstructorName => KeyDef {
                name: "instructor_name",
                exprs: &["(SELECT i.display_name FROM course_instructors ci \
                        JOIN instructors i ON i.id = ci.instructor_id \
                        WHERE ci.course_id = courses.id AND ci.is_primary = true LIMIT 1)"],
                asc: "Name, A to Z",
                desc: "Name, Z to A",
            },
            // sort_score is the CI lower bound, which is what ratings are ranked on.
            Self::InstructorRating => KeyDef {
                name: "instructor_rating",
                exprs: &["(SELECT s.sort_score FROM course_instructors ci \
                        JOIN instructor_scores s ON s.instructor_id = ci.instructor_id \
                        WHERE ci.course_id = courses.id AND ci.is_primary = true LIMIT 1)"],
                asc: "Lowest rated",
                desc: "Highest rated",
            },
            Self::StartTime => KeyDef {
                name: "start_time",
                exprs: &["first_begin_minutes"],
                asc: "Earliest first",
                desc: "Latest first",
            },
            Self::EndTime => KeyDef {
                name: "end_time",
                exprs: &["first_end_minutes"],
                asc: "Ends earliest",
                desc: "Ends latest",
            },
            Self::Duration => KeyDef {
                name: "duration",
                exprs: &["meeting_minutes"],
                asc: "Shortest first",
                desc: "Longest first",
            },
            Self::WeeklyMinutes => KeyDef {
                name: "weekly_minutes",
                exprs: &["weekly_minutes"],
                asc: "Least time per week",
                desc: "Most time per week",
            },
            // The mask is Monday-first bit order, so ascending groups Monday
            // patterns before Tuesday ones rather than ordering by day count.
            Self::Days => KeyDef {
                name: "days",
                exprs: &["day_mask"],
                asc: "Earliest weekday first",
                desc: "Latest weekday first",
            },
            Self::SeatsOpen => KeyDef {
                name: "seats_open",
                exprs: &["(max_enrollment - enrollment)"],
                asc: "Nearly full",
                desc: "Most seats open",
            },
            // Guarded against the sections whose max enrollment is zero.
            Self::FillRatio => KeyDef {
                name: "fill_ratio",
                exprs: &["(CASE WHEN max_enrollment > 0 \
                        THEN enrollment::float8 / max_enrollment ELSE NULL END)"],
                asc: "Emptiest first",
                desc: "Fullest first",
            },
            Self::WaitCount => KeyDef {
                name: "wait_count",
                exprs: &["wait_count"],
                asc: "Shortest waitlist",
                desc: "Longest waitlist",
            },
        }
    }

    /// Wire name, e.g. `start_time`.
    pub const fn name(self) -> &'static str {
        self.def().name
    }

    /// What this ordering means to a reader, for a header tooltip or a menu.
    pub const fn label(self, direction: SortDirection) -> &'static str {
        let def = self.def();
        match direction {
            SortDirection::Asc => def.asc,
            SortDirection::Desc => def.desc,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|key| key.name() == name)
    }

    fn describe(self) -> SortKeyOption {
        SortKeyOption {
            key: self,
            asc_label: self.label(SortDirection::Asc).to_owned(),
            desc_label: self.label(SortDirection::Desc).to_owned(),
        }
    }

    /// Every key with its labels, so the client names a sort without restating it.
    pub fn catalog() -> Vec<SortKeyOption> {
        Self::ALL.iter().copied().map(Self::describe).collect()
    }
}

/// A sort key as offered to the client: what it is, and what each way round means.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SortKeyOption {
    pub key: SortKey,
    pub asc_label: String,
    pub desc_label: String,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    Desc,
}

/// Catalog order, and the tiebreaker every sort ends on.
const DEFAULT_ORDER: &str = "subject ASC, course_number ASC, sequence_number ASC";

/// Terms beyond this are dropped: a deeper sort cannot change the order, since
/// the tiebreaker is already total, but it does cost a sort pass each.
const MAX_SORT_TERMS: usize = 4;

/// One key ordered one way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortTerm {
    pub key: SortKey,
    pub direction: SortDirection,
}

impl SortTerm {
    fn to_sql(self) -> String {
        let dir = match self.direction {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        };
        // NULLS LAST in both directions: a section with no value for the key is
        // not "lowest", it is absent, and reversing should not float it to page one.
        self.key
            .def()
            .exprs
            .iter()
            .map(|expr| format!("{expr} {dir} NULLS LAST"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl fmt::Display for SortTerm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if matches!(self.direction, SortDirection::Desc) {
            f.write_str("-")?;
        }
        f.write_str(self.key.name())
    }
}

impl FromStr for SortTerm {
    type Err = SortParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let (direction, name) = match s.strip_prefix('-') {
            Some(rest) => (SortDirection::Desc, rest),
            None => (SortDirection::Asc, s),
        };
        SortKey::from_name(name)
            .map(|key| SortTerm { key, direction })
            .ok_or_else(|| SortParseError::UnknownKey(name.to_owned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortParseError {
    UnknownKey(String),
}

impl fmt::Display for SortParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKey(name) => {
                let known: Vec<&str> = SortKey::ALL.iter().map(|k| k.name()).collect();
                write!(
                    f,
                    "unknown sort key '{name}'; expected one of {}",
                    known.join(", ")
                )
            }
        }
    }
}

impl std::error::Error for SortParseError {}

/// An ordered sort.
///
/// The catalog tiebreaker is appended by `to_sql` rather than stored, so no
/// caller can build a spec that pages unstably. An empty spec is catalog order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SortSpec(Vec<SortTerm>);

impl SortSpec {
    pub fn new(mut terms: Vec<SortTerm>) -> Self {
        terms.truncate(MAX_SORT_TERMS);
        Self(terms)
    }

    /// The ORDER BY body. Every fragment is a hardcoded literal from `KeyDef`,
    /// so nothing a caller sends can reach the SQL text.
    pub fn to_sql(&self) -> String {
        let mut parts: Vec<String> = self.0.iter().map(|term| term.to_sql()).collect();
        parts.push(DEFAULT_ORDER.to_owned());
        parts.join(", ")
    }
}

impl fmt::Display for SortSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, term) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(",")?;
            }
            write!(f, "{term}")?;
        }
        Ok(())
    }
}

impl FromStr for SortSpec {
    type Err = SortParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let terms = s
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(SortTerm::from_str)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Self::new(terms))
    }
}

#[cfg(test)]
mod sort_tests {
    use super::*;

    #[test]
    fn wire_names_match_serde() {
        for key in SortKey::ALL {
            let json = serde_json::to_string(key).expect("key serializes");
            assert_eq!(json, format!("\"{}\"", key.name()), "{key:?} drifted");
        }
    }

    #[test]
    fn every_key_is_reachable_by_name() {
        for key in SortKey::ALL {
            assert_eq!(SortKey::from_name(key.name()), Some(*key));
        }
        assert_eq!(SortKey::from_name("nope"), None);
    }

    /// Pins the variant count so a new variant is noticed here even though
    /// strum, not a hand-maintained list, is what keeps ALL exhaustive.
    #[test]
    fn all_holds_every_variant() {
        assert_eq!(SortKey::ALL.len(), 12);
    }

    #[test]
    fn terms_round_trip_through_the_wire_format() {
        let raw = "start_time,-instructor_rating,duration";
        let spec: SortSpec = raw.parse().expect("parses");
        assert_eq!(spec.to_string(), raw);
    }

    #[test]
    fn the_leading_term_drives_the_ordering() {
        let asc = "start_time".parse::<SortSpec>().expect("parses").to_sql();
        let desc = "-start_time".parse::<SortSpec>().expect("parses").to_sql();
        assert!(
            asc.starts_with("first_begin_minutes ASC NULLS LAST"),
            "{asc}"
        );
        assert!(
            desc.starts_with("first_begin_minutes DESC NULLS LAST"),
            "{desc}"
        );
    }

    #[test]
    fn an_unknown_key_names_itself_and_the_alternatives() {
        let err = "start_time,bogus".parse::<SortSpec>().unwrap_err();
        let message = err.to_string();
        assert!(message.contains("bogus"), "{message}");
        assert!(message.contains("start_time"), "{message}");
    }

    /// The tiebreaker is what keeps offset paging stable across shared keys, so
    /// no spec may omit it -- including the empty one.
    #[test]
    fn every_spec_ends_on_the_tiebreaker() {
        assert!(SortSpec::default().to_sql().ends_with(DEFAULT_ORDER));
        for key in SortKey::ALL {
            for direction in [SortDirection::Asc, SortDirection::Desc] {
                let spec = SortSpec::new(vec![SortTerm {
                    key: *key,
                    direction,
                }]);
                assert!(
                    spec.to_sql().ends_with(DEFAULT_ORDER),
                    "{key:?} {direction:?}"
                );
            }
        }
    }

    /// A key spanning several columns must reverse all of them, not just the last.
    #[test]
    fn a_multi_column_key_reverses_every_column() {
        let sql = "-course_code".parse::<SortSpec>().expect("parses").to_sql();
        let leading = sql
            .strip_suffix(&format!(", {DEFAULT_ORDER}"))
            .expect("ends on the tiebreaker");
        for column in SortKey::CourseCode.def().exprs {
            assert!(
                leading.contains(&format!("{column} DESC NULLS LAST")),
                "{column} is not descending in {leading}"
            );
        }
        assert!(!leading.contains("ASC"), "{leading}");
    }

    /// Absence is not a low value: reversing a sort must not float unscheduled
    /// sections to the first page.
    #[test]
    fn null_rows_sink_in_both_directions() {
        for direction in [SortDirection::Asc, SortDirection::Desc] {
            let spec = SortSpec::new(vec![SortTerm {
                key: SortKey::StartTime,
                direction,
            }]);
            assert!(spec.to_sql().contains("NULLS LAST"));
        }
    }

    #[test]
    fn extra_terms_are_dropped_rather_than_sorted_on() {
        let raw = "start_time,end_time,duration,weekly_minutes,days,seats_open";
        let spec: SortSpec = raw.parse().expect("parses");
        assert_eq!(spec.to_string().split(',').count(), MAX_SORT_TERMS);
    }

    #[test]
    fn labels_never_say_ascending() {
        for key in SortKey::ALL {
            for direction in [SortDirection::Asc, SortDirection::Desc] {
                let label = key.label(direction).to_lowercase();
                assert!(!label.contains("ascend"), "{key:?}: {label}");
                assert!(!label.contains("descend"), "{key:?}: {label}");
            }
        }
    }
}
