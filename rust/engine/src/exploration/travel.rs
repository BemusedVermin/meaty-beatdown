//! Ocean travel + the engage-an-encounter handoff. Pure hex logic; the Bevy shell (`app`) drives it
//! from `ExplorationState::Overworld`. Sailing into a visible encounter hands the encounter back to
//! the caller, who raises the combat overlay — there is no preview/confirm (the fight just begins).

use super::encounter::Encounter;
use super::hex::{distance, Hex};
use super::poi::Poi;
use super::world::World;

/// The player's ship/party position on the hexcrawl.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Party {
    pub pos: Hex,
}

/// The outcome of attempting to sail to an adjacent hex.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TravelResult {
    /// Sailed onto open water; `cost` is the terrain-dependent effort spent.
    Sailed { to: Hex, cost: u32 },
    /// Sailed into a visible encounter → hand off to combat (no preview).
    Engaged { at: Hex, encounter: Encounter },
    /// Reached land — dock at the island (and its POI, if any).
    Docked { at: Hex, poi: Option<Poi> },
    /// Not a legal step (off-map, or not adjacent to the party).
    Blocked,
}

/// Attempt to sail the party to `to` — which must be a neighbour of `party.pos`. On a legal step the
/// party position is updated (including when engaging or docking). Encounter takes priority over
/// terrain, so sailing onto an encounter's hex always engages.
pub fn sail(world: &World, party: &mut Party, to: Hex) -> TravelResult {
    if !is_neighbor(party.pos, to) || world.tile(to).is_none() {
        return TravelResult::Blocked;
    }
    if let Some(&encounter) = world.encounter(to) {
        party.pos = to;
        return TravelResult::Engaged { at: to, encounter };
    }
    // `tile` is present (checked above).
    let terrain = world.tile(to).unwrap().terrain;
    party.pos = to;
    if terrain.is_land() {
        TravelResult::Docked { at: to, poi: world.poi(to).copied() }
    } else {
        TravelResult::Sailed { to, cost: terrain.travel_cost() }
    }
}

/// The adjacent hexes the party may move into (those that exist on the map).
pub fn navigable_neighbors(world: &World, from: Hex) -> Vec<Hex> {
    from.all_neighbors().into_iter().filter(|h| world.tile(*h).is_some()).collect()
}

/// Encounters within `sight` hexes of `from`. (Every encounter is visible; this is just a range cull
/// for the local view.)
pub fn visible_encounters(world: &World, from: Hex, sight: i32) -> Vec<(Hex, Encounter)> {
    world
        .encounters
        .iter()
        .filter(|(h, _)| distance(from, **h) <= sight)
        .map(|(h, e)| (*h, *e))
        .collect()
}

fn is_neighbor(a: Hex, b: Hex) -> bool {
    a.all_neighbors().contains(&b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::poi::{Faction, Poi};
    use super::super::terrain::Terrain;
    use super::super::world::{Tile, World};
    use std::collections::HashMap;

    /// A tiny hand-built world: the origin + its 6 neighbours, with one land+port and one encounter.
    fn tiny() -> (World, Hex, [Hex; 6]) {
        let p = Hex::new(0, 0);
        let nbrs = p.all_neighbors();
        let mut tiles = HashMap::new();
        tiles.insert(p, Tile { terrain: Terrain::Deep, faction: None });
        for &n in &nbrs {
            tiles.insert(n, Tile { terrain: Terrain::Deep, faction: None });
        }
        tiles.insert(nbrs[2], Tile { terrain: Terrain::Island, faction: None }); // land
        let mut pois = HashMap::new();
        pois.insert(nbrs[2], Poi::Port { faction: Faction::Concord });
        let mut encounters = HashMap::new();
        encounters.insert(nbrs[1], Encounter { faction: Faction::TidemarkHosts, strength: 1 });
        let world = World { seed: 0, radius: 2, tiles, pois, encounters };
        (world, p, nbrs)
    }

    #[test]
    fn sailing_open_water() {
        let (world, p, nbrs) = tiny();
        let mut party = Party { pos: p };
        assert!(matches!(sail(&world, &mut party, nbrs[0]), TravelResult::Sailed { .. }));
        assert_eq!(party.pos, nbrs[0]);
    }

    #[test]
    fn sailing_into_an_encounter_engages() {
        let (world, p, nbrs) = tiny();
        let mut party = Party { pos: p };
        match sail(&world, &mut party, nbrs[1]) {
            TravelResult::Engaged { at, encounter } => {
                assert_eq!(at, nbrs[1]);
                assert_eq!(encounter.faction, Faction::TidemarkHosts);
            }
            other => panic!("expected Engaged, got {other:?}"),
        }
    }

    #[test]
    fn sailing_to_land_docks_at_the_port() {
        let (world, p, nbrs) = tiny();
        let mut party = Party { pos: p };
        match sail(&world, &mut party, nbrs[2]) {
            TravelResult::Docked { poi: Some(Poi::Port { .. }), .. } => {}
            other => panic!("expected Docked at a port, got {other:?}"),
        }
    }

    #[test]
    fn cannot_jump_to_a_far_hex() {
        let (world, p, _) = tiny();
        let mut party = Party { pos: p };
        assert_eq!(sail(&world, &mut party, Hex::new(9, 9)), TravelResult::Blocked);
        assert_eq!(party.pos, p); // position unchanged on a blocked move
    }
}
