//! Matchup knowledge (spec §7.3): the fog as progression. Per-move tiers gate how much
//! an Observation's cue views are enriched. EARNING tiers is the campaign's business
//! (progression.md, Phase 9); a fight receives a side's book as read-only input.
//!
//! Knowledge never removes the read — a T3 candidate set with two entries is still a
//! guess — it sharpens it.

use super::MoveId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum KnowledgeTier {
    /// T0 — the cue shows as its generic silhouette class.
    #[default]
    Unknown,
    /// T1 — seen it resolve: named; height/category class shown (codex-side).
    Glimpsed,
    /// T2 — studied: full frame data known; a matching cue overlays the candidate set.
    Studied,
    /// T3 — mastered: exact phase-tick readout; throw break keys shown on grab cues.
    Mastered,
}

/// One side's knowledge of moves it may face. Default: everything Unknown.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeBook(pub BTreeMap<MoveId, KnowledgeTier>);

impl KnowledgeBook {
    #[must_use]
    pub fn tier(&self, id: MoveId) -> KnowledgeTier {
        self.0.get(&id).copied().unwrap_or_default()
    }

    /// Convenience: a book holding every listed move at one tier.
    #[must_use]
    pub fn uniform(moves: impl IntoIterator<Item = MoveId>, tier: KnowledgeTier) -> Self {
        Self(moves.into_iter().map(|m| (m, tier)).collect())
    }
}
