//! Interned newtype identifiers (tech-plan §5).
//!
//! Stable `EntityId` ordering is a determinism rule: same-tick effects resolve in entity-id
//! order (spec §4.2), so these are `Ord` by construction.

use serde::{Deserialize, Serialize};

/// An actor in a fight (player character, companion, enemy, projectile entity).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityId(pub u32);

/// A side in a fight; sides drive win/loss (a side loses when all its actors are KO'd).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SideId(pub u8);
