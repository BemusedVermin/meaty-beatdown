//! Points of interest + factions, drawn from the plot bible (`docs/the-promise-plot-bible.md`).

/// The five great powers (plus the unaligned). Tags islands, ports, and sects.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Faction {
    /// The orthodoxy that administers the Ascent and venerates the Crossing.
    Concord,
    /// The warlord state — the Forms as weapons, the Promise as a recruiting poster.
    TidemarkHosts,
    /// The reformist "heretics" who preach ceasing to strive.
    Stillwater,
    /// The hidden collaborators who feed the harvest to hold back the deluge.
    Ferrymen,
    /// The dispossessed refugees of the Tide.
    Drowned,
    Unaligned,
}

/// A point of interest sitting on a land hex.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Poi {
    /// A merchant harbour — the Pirates-of-the-Caribbean port town.
    Port { faction: Faction },
    /// A sect hall — skill trainers teaching the Forms.
    SectHall { faction: Faction },
    /// A master's seat — a peak Ascendant whose presence holds the deluge back here.
    MasterSeat { faction: Faction },
    /// A drowned ruin to delve (the dungeon loop).
    Ruin,
    /// A refugee camp of the Drowned.
    DrownedCamp,
    /// A Threshold — where Ascendants Cross (and are harvested).
    Threshold,
}

impl Poi {
    pub fn faction(self) -> Faction {
        match self {
            Poi::Port { faction } | Poi::SectHall { faction } | Poi::MasterSeat { faction } => {
                faction
            }
            _ => Faction::Unaligned,
        }
    }
}
