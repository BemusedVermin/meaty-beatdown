//! Procedural world generation — WWN's sandbox method (seed-and-spread terrain, then scale locations
//! by land area), adapted for a 95%-water world. Seeded by a `u64` world seed via `fastrand`, so a
//! seed reproduces the same world.
//!
//! Steps: start with open ocean → place a few **master seats** spread far apart (the masters that
//! hold the deluge back) → grow an **archipelago** of land around each → scatter lone islets + a few
//! hazards to reach the land target → place POIs, scaling count with land area and guaranteeing at
//! least one merchant (`Port`) and one trainer (`SectHall`).

use super::hex::{map_hexes, Hex};
use super::poi::{Faction, Poi};
use super::terrain::Terrain;
use super::world::{Tile, World};
use std::collections::HashMap;

pub struct GenConfig {
    pub radius: i32,
    /// Number of master seats / archipelagos.
    pub masters: u32,
    /// Target fraction of land (~0.05 for a 95%-water world).
    pub land_target: f32,
}
impl Default for GenConfig {
    fn default() -> Self {
        Self { radius: 14, masters: 5, land_target: 0.05 }
    }
}

const FACTIONS: [Faction; 5] = [
    Faction::Concord,
    Faction::TidemarkHosts,
    Faction::Stillwater,
    Faction::Ferrymen,
    Faction::Drowned,
];

/// Generate a world from a seed + config.
pub fn generate(seed: u64, cfg: &GenConfig) -> World {
    let mut rng = fastrand::Rng::with_seed(seed);
    let all = map_hexes(cfg.radius);

    // 1. All open ocean.
    let mut tiles: HashMap<Hex, Tile> =
        all.iter().map(|&h| (h, Tile { terrain: Terrain::Deep, faction: None })).collect();
    let mut pois: HashMap<Hex, Poi> = HashMap::new();

    // 2. Master seats, spread far apart; each anchors an archipelago.
    let seats = pick_spread(&all, cfg.masters as usize, &mut rng);
    for (i, &seat) in seats.iter().enumerate() {
        let faction = FACTIONS[i % FACTIONS.len()];
        grow_archipelago(&mut tiles, seat, faction, &mut rng);
        if let Some(t) = tiles.get_mut(&seat) {
            t.terrain = Terrain::Island;
            t.faction = Some(faction);
        }
        pois.insert(seat, Poi::MasterSeat { faction });
    }

    // 3. Scatter lone islets + hazards toward the land target.
    scatter(&mut tiles, &all, cfg, &mut rng);

    // 4. Place POIs on land, scaled by area, guaranteeing merchants + trainers.
    place_pois(&mut tiles, &mut pois, &mut rng);

    World { seed, radius: cfg.radius, tiles, pois }
}

/// Axial hex distance.
fn dist(a: Hex, b: Hex) -> i32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx.abs() + (dx + dy).abs() + dy.abs()) / 2
}

/// Pick `n` distinct hexes greedily spread apart (each new one maximises the min distance to the set).
fn pick_spread(all: &[Hex], n: usize, rng: &mut fastrand::Rng) -> Vec<Hex> {
    let mut chosen = Vec::new();
    if all.is_empty() || n == 0 {
        return chosen;
    }
    chosen.push(all[rng.usize(0..all.len())]);
    while chosen.len() < n {
        let mut best: Option<Hex> = None;
        let mut best_d = -1;
        for _ in 0..32 {
            let c = all[rng.usize(0..all.len())];
            if chosen.contains(&c) {
                continue;
            }
            let d = chosen.iter().map(|&s| dist(c, s)).min().unwrap_or(0);
            if d > best_d {
                best_d = d;
                best = Some(c);
            }
        }
        match best {
            Some(c) => chosen.push(c),
            // fallback: first unchosen hex deterministically
            None => match all.iter().find(|h| !chosen.contains(h)) {
                Some(&c) => chosen.push(c),
                None => break,
            },
        }
    }
    chosen
}

/// Grow a small archipelago around `seat`: a blob of Island hexes ringed with Shallows.
fn grow_archipelago(
    tiles: &mut HashMap<Hex, Tile>,
    seat: Hex,
    faction: Faction,
    rng: &mut fastrand::Rng,
) {
    let target = rng.usize(3..=8);
    let mut land = vec![seat];
    let mut guard = 0;
    while land.len() < target && guard < 200 {
        guard += 1;
        let from = land[rng.usize(0..land.len())];
        let n = from.all_neighbors()[rng.usize(0..6)];
        if let Some(t) = tiles.get_mut(&n) {
            if t.terrain != Terrain::Island {
                t.terrain = Terrain::Island;
                t.faction = Some(faction);
                land.push(n);
            }
        }
    }
    // Ring of shallows around the land.
    for &l in &land {
        for nb in l.all_neighbors() {
            if let Some(t) = tiles.get_mut(&nb) {
                if t.terrain == Terrain::Deep {
                    t.terrain = Terrain::Shallows;
                }
            }
        }
    }
}

/// Top up lone islets to the land target, then sprinkle a few reefs/storms.
fn scatter(tiles: &mut HashMap<Hex, Tile>, all: &[Hex], cfg: &GenConfig, rng: &mut fastrand::Rng) {
    let total = all.len();
    let land_target = (total as f32 * cfg.land_target) as usize;
    let mut land = tiles.values().filter(|t| t.terrain == Terrain::Island).count();
    let mut guard = 0;
    while land < land_target && guard < total * 4 {
        guard += 1;
        let h = all[rng.usize(0..total)];
        if let Some(t) = tiles.get_mut(&h) {
            if t.terrain == Terrain::Deep {
                t.terrain = Terrain::Island; // a lone islet
                land += 1;
                for nb in h.all_neighbors() {
                    if let Some(s) = tiles.get_mut(&nb) {
                        if s.terrain == Terrain::Deep {
                            s.terrain = Terrain::Shallows;
                        }
                    }
                }
            }
        }
    }
    for _ in 0..(total / 40) {
        let h = all[rng.usize(0..total)];
        if let Some(t) = tiles.get_mut(&h) {
            match t.terrain {
                Terrain::Shallows => t.terrain = Terrain::Reef,
                Terrain::Deep => t.terrain = Terrain::Storm,
                _ => {}
            }
        }
    }
}

/// Place POIs on empty land hexes (count scales with land area), then guarantee a merchant + trainer.
fn place_pois(tiles: &mut HashMap<Hex, Tile>, pois: &mut HashMap<Hex, Poi>, rng: &mut fastrand::Rng) {
    let mut land = empty_land(tiles, pois);
    rng.shuffle(&mut land);

    let mut ports = 0;
    let mut sects = 0;
    for h in land {
        let f = tiles.get(&h).and_then(|t| t.faction).unwrap_or(Faction::Unaligned);
        let poi = match rng.u32(0..100) {
            0..=29 => {
                ports += 1;
                Poi::Port { faction: f }
            }
            30..=49 => {
                sects += 1;
                Poi::SectHall { faction: f }
            }
            50..=67 => Poi::Ruin,
            68..=81 => Poi::DrownedCamp,
            82..=91 => Poi::Threshold,
            _ => continue, // some islands stay empty
        };
        pois.insert(h, poi);
    }

    if ports == 0 {
        ensure_one(tiles, pois, |f| Poi::Port { faction: f });
    }
    if sects == 0 {
        ensure_one(tiles, pois, |f| Poi::SectHall { faction: f });
    }
}

/// Empty land hexes, in deterministic coordinate order.
fn empty_land(tiles: &HashMap<Hex, Tile>, pois: &HashMap<Hex, Poi>) -> Vec<Hex> {
    let mut land: Vec<Hex> = tiles
        .iter()
        .filter(|(h, t)| t.terrain == Terrain::Island && !pois.contains_key(h))
        .map(|(h, _)| *h)
        .collect();
    land.sort_by_key(|h| (h.x, h.y));
    land
}

fn ensure_one(
    tiles: &HashMap<Hex, Tile>,
    pois: &mut HashMap<Hex, Poi>,
    make: impl Fn(Faction) -> Poi,
) {
    if let Some(&h) = empty_land(tiles, pois).first() {
        let f = tiles.get(&h).and_then(|t| t.faction).unwrap_or(Faction::Unaligned);
        pois.insert(h, make(f));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_is_mostly_water() {
        let w = generate(42, &GenConfig::default());
        assert!(w.water_fraction() > 0.9, "water = {}", w.water_fraction());
    }

    #[test]
    fn has_the_master_seats() {
        let cfg = GenConfig::default();
        let w = generate(42, &cfg);
        let seats = w.pois.values().filter(|p| matches!(p, Poi::MasterSeat { .. })).count();
        assert_eq!(seats, cfg.masters as usize);
    }

    #[test]
    fn has_merchants_and_trainers() {
        let w = generate(7, &GenConfig::default());
        assert!(w.pois.values().any(|p| matches!(p, Poi::Port { .. })), "needs a merchant");
        assert!(w.pois.values().any(|p| matches!(p, Poi::SectHall { .. })), "needs a trainer");
    }

    #[test]
    fn pois_sit_on_land() {
        let w = generate(123, &GenConfig::default());
        for h in w.pois.keys() {
            assert_eq!(w.tiles[h].terrain, Terrain::Island);
        }
    }

    #[test]
    fn reproducible_from_seed() {
        let a = generate(999, &GenConfig::default());
        let b = generate(999, &GenConfig::default());
        assert_eq!(a.tiles, b.tiles);
        assert_eq!(a.pois, b.pois);
    }
}
