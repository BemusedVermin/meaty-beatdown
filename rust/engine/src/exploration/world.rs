//! The generated world: a hex map of terrain + POIs.

use super::hex::Hex;
use super::poi::{Faction, Poi};
use super::terrain::Terrain;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tile {
    pub terrain: Terrain,
    /// Which power, if any, controls this hex.
    pub faction: Option<Faction>,
}

/// A generated flooded-world hexcrawl.
#[derive(Clone, Debug)]
pub struct World {
    pub seed: u64,
    pub radius: i32,
    pub tiles: HashMap<Hex, Tile>,
    pub pois: HashMap<Hex, Poi>,
}

impl World {
    pub fn tile(&self, h: Hex) -> Option<&Tile> {
        self.tiles.get(&h)
    }
    pub fn poi(&self, h: Hex) -> Option<&Poi> {
        self.pois.get(&h)
    }
    pub fn count_terrain(&self, t: Terrain) -> usize {
        self.tiles.values().filter(|tile| tile.terrain == t).count()
    }
    /// Fraction of the map that is water (the world should be ~95%).
    pub fn water_fraction(&self) -> f32 {
        let total = self.tiles.len().max(1);
        let water = self.tiles.values().filter(|t| t.terrain.is_water()).count();
        water as f32 / total as f32
    }
    /// POIs matching a predicate (e.g. all ports / all sect halls).
    pub fn pois_where<'a>(
        &'a self,
        pred: impl Fn(&Poi) -> bool + 'a,
    ) -> impl Iterator<Item = (&'a Hex, &'a Poi)> {
        self.pois.iter().filter(move |(_, p)| pred(p))
    }
}
