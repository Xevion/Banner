use serde::{Deserialize, Serialize};

/// Represents a key-value pair from the Banner API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pair {
    pub code: String,
    pub description: String,
}

/// Represents a term in the Banner system
pub type BannerTerm = Pair;

impl BannerTerm {
    /// Returns true if the term is in an archival (view-only) state
    pub fn is_archived(&self) -> bool {
        self.description.contains("View Only")
    }
}
