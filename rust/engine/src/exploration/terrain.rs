//! Terrain for the flooded world — overwhelmingly water. The Tide drowned everything; only the
//! masters' seats keep islands above it.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Terrain {
    /// Open ocean — the default; fast, safe sailing.
    Deep,
    /// Coastal water around land — slower, where ports sit.
    Shallows,
    /// A navigable hazard — reefs that tear hulls.
    Reef,
    /// A roving ocean storm — a moving hazard.
    Storm,
    /// Rare land — where everything that isn't sailing happens (the ~5%).
    Island,
}

impl Terrain {
    pub fn is_water(self) -> bool {
        !matches!(self, Terrain::Island)
    }
    pub fn is_land(self) -> bool {
        matches!(self, Terrain::Island)
    }
    /// Relative effort to sail across this hex (land is a destination, not sailed through).
    pub fn travel_cost(self) -> u32 {
        match self {
            Terrain::Deep => 1,
            Terrain::Shallows => 2,
            Terrain::Reef | Terrain::Storm => 3,
            Terrain::Island => 0,
        }
    }
}
