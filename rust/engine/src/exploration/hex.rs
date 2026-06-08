//! Hex coordinates for the ocean hexcrawl — axial coords via the `hexx` library.

pub use hexx::Hex;

/// Every hex within `radius` of the origin — a hexagonal map (the standard axial disk).
pub fn map_hexes(radius: i32) -> Vec<Hex> {
    let mut out = Vec::new();
    for x in -radius..=radius {
        let lo = (-radius).max(-x - radius);
        let hi = radius.min(-x + radius);
        for y in lo..=hi {
            out.push(Hex::new(x, y));
        }
    }
    out
}
