//! Confidence scoring and candidate generation for RMP instructor matching.

mod abbreviations;
mod pipeline;
mod score;

// The bin target recompiles this tree separately from the lib target and does
// not itself use every re-export; the lib's external consumers (tests) do.
pub use pipeline::{MatchingStats, generate_candidates};
pub use score::{MatchScore, ScoreBreakdown, compute_match_score};
