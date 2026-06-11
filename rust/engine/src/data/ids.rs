//! Interned content identifiers (tech-plan §5).

use serde::{Deserialize, Serialize};

/// An authored move. Every move belongs to a Form (C-AUTH: no generic moves).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MoveId(pub u32);

/// A Form — a moveset identity taught by masters (spec §12; progression.md).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FormId(pub u32);
