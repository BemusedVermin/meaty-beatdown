//! The compiler: a [`Build`] → a [`Fighter`] (body + moves). The single L4 → L2 bridge — wires the
//! one-lever-per-attribute design (spec R-2): CON→HP, DEX→startup, STR→damage, skill→frame safety.
//! (INT/WIS/CHA drive Focus/tempo resources the engine doesn't run yet — deferred, not wired here.)

use super::moves::{self, Levers};
use super::sheet::Build;
use crate::fighting::{Body, BodyPart, Entity, FrameProfile, Health, MoveId, MoveTable, Reaction, SideId, Sim, Vec3};

const BASE_HP: i32 = 80;
const HP_PER_CON: i32 = 10;
const DEX_STARTUP_CAP: i32 = 3;

/// A compiled fighter: the body it presents + the moves it brings to a fight.
#[derive(Clone, Debug)]
pub struct Fighter {
    pub body: Body,
    pub health: Health,
    pub moves: Vec<FrameProfile>,
}

impl Fighter {
    /// Spawn a runtime [`Entity`] for this fighter at a side / position / facing.
    pub fn entity(&self, side: SideId, pos: Vec3, facing: i8) -> Entity {
        Entity {
            side,
            pos,
            facing,
            body: self.body.clone(),
            health: self.health,
            ready_tick: 0,
            action: None,
            reaction: Reaction::Neutral,
        }
    }
}

/// Compile a build into a fighter: morphology → body, modifiers → extra parts, stats/skills/weapon →
/// the move set (the combat unit).
pub fn compile(build: &Build) -> Fighter {
    // 1. Body: the morphology's stick figure, then attach each compatible modifier (adds a part).
    let mut body = build.morphology.body();
    let base_parts = build.morphology.parts();
    for &m in &build.modifiers {
        if m.compatible_with(base_parts) {
            if let Some(base_box) = body.part(m.requires()) {
                body.parts.push((m.adds(), m.added_box(base_box)));
            }
        }
    }

    // 2. Health from CON.
    let a = &build.attributes;
    let health = (BASE_HP + a.con_mod() * HP_PER_CON).max(1) as Health;

    // 3. The wired levers.
    let lv = Levers {
        startup_cut: a.dex_mod().clamp(0, DEX_STARTUP_CAP),
        recovery_cut: (build.skills.weapon.max(build.skills.unarmed) as i32) / 2,
        damage_bonus: 2 * a.str_mod(),
    };

    // 4. Moves: natural strikes gated by the parts the body actually has, + weapon (if wielded & STR met).
    let mut mv = Vec::new();
    if body.has(BodyPart::Fist) {
        mv.push(moves::punch(&lv));
    }
    if body.has(BodyPart::Foot) {
        mv.push(moves::kick(&lv));
    }
    if body.has(BodyPart::Fangs) {
        mv.push(moves::bite(&lv));
    }
    if body.has(BodyPart::Claws) {
        mv.push(moves::claw(&lv));
    }
    if let Some(w) = &build.weapon {
        if body.has(BodyPart::Fist) && a.strength >= w.req_str {
            mv.extend(moves::weapon_moves(w, &lv));
        }
    }
    // Universal defense, added last so its MoveId sits above the strikes — the placeholder AI picks
    // the lowest id, so it keeps attacking rather than turtling.
    mv.push(moves::guard(&lv));

    Fighter { body, health, moves: mv }
}

/// Assemble a runnable [`Sim`] from a player fighter vs. a set of foes (the encounter → fight bridge).
/// All moves merge into one shared table; each entity may use only those its body allows (the engine
/// morphology gate). Positions are a footsies-range default — fight setup can place them otherwise.
pub fn arena(player: &Fighter, foes: &[Fighter]) -> Sim {
    let mut table = MoveTable::new();
    let mut next = 1u32;
    add_moves(&mut table, &mut next, player);
    for f in foes {
        add_moves(&mut table, &mut next, f);
    }

    let mut entities = vec![player.entity(SideId(0), Vec3::new(-1.5, 0.0, 0.0), 1)];
    for (i, f) in foes.iter().enumerate() {
        entities.push(f.entity(SideId(1), Vec3::new(1.5 + i as f32, 0.0, 0.0), -1));
    }

    Sim::new(entities, table)
}

fn add_moves(table: &mut MoveTable, next: &mut u32, f: &Fighter) {
    for p in &f.moves {
        table.insert(MoveId(*next), p.clone());
        *next += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::super::attributes::Attributes;
    use super::super::generate::generate_fighter;
    use super::super::morphology::{Modifier, Morphology};
    use super::super::sheet::{Build, Skills};
    use super::*;
    use crate::fighting::{HitboxSource, QualityKind};

    fn avg() -> Attributes {
        Attributes { strength: 10, dexterity: 10, constitution: 10, intelligence: 10, wisdom: 10, charisma: 10 }
    }
    fn build(morphology: Morphology, modifiers: Vec<Modifier>, attributes: Attributes) -> Build {
        Build { attributes, morphology, modifiers, skills: Skills::default(), weapon: None }
    }
    /// The damage of the punch move (the Fist-sourced strike), if present.
    fn punch_damage(f: &Fighter) -> Option<u32> {
        f.moves.iter().find_map(|m| match &m.qualities[0].kind {
            QualityKind::Hitbox(a) if matches!(a.source, HitboxSource::Part(BodyPart::Fist)) => {
                Some(a.hit.damage)
            }
            _ => None,
        })
    }

    #[test]
    fn biped_punches_quadruped_cannot() {
        let biped = compile(&build(Morphology::Biped, vec![], avg()));
        let quad = compile(&build(Morphology::Quadruped, vec![], avg()));
        assert!(biped.body.has(BodyPart::Fist));
        assert!(!quad.body.has(BodyPart::Fist));
        assert!(biped.moves.iter().any(|m| m.requires.contains(&BodyPart::Fist)));
        assert!(!quad.moves.iter().any(|m| m.requires.contains(&BodyPart::Fist)));
    }

    #[test]
    fn clawed_needs_an_arm_but_fanged_does_not() {
        // A quadruped (no arm) can't be clawed, but can be fanged (it has a head).
        let quad_clawed = compile(&build(Morphology::Quadruped, vec![Modifier::Clawed], avg()));
        assert!(!quad_clawed.body.has(BodyPart::Claws));
        let quad_fanged = compile(&build(Morphology::Quadruped, vec![Modifier::Fanged], avg()));
        assert!(quad_fanged.body.has(BodyPart::Fangs));
        assert!(quad_fanged.moves.iter().any(|m| m.requires.contains(&BodyPart::Fangs)));
    }

    #[test]
    fn strength_scales_move_damage() {
        let weak = compile(&build(Morphology::Biped, vec![], Attributes { strength: 3, ..avg() }));
        let strong = compile(&build(Morphology::Biped, vec![], Attributes { strength: 18, ..avg() }));
        assert!(punch_damage(&strong).unwrap() > punch_damage(&weak).unwrap());
    }

    #[test]
    fn constitution_scales_health() {
        let frail = compile(&build(Morphology::Biped, vec![], Attributes { constitution: 3, ..avg() }));
        let tough = compile(&build(Morphology::Biped, vec![], Attributes { constitution: 18, ..avg() }));
        assert!(tough.health > frail.health);
    }

    #[test]
    fn arena_builds_a_runnable_sim() {
        let a = generate_fighter(1);
        let b = generate_fighter(2);
        let sim = arena(&a, &[b]);
        assert_eq!(sim.entities.len(), 2);
        assert!(!sim.moves.is_empty());
    }

    /// The **no-infinite-combo invariant**: every authored strike's hit-/block-stun must end before
    /// the same move could re-connect (strictly less than the move's own `total`), so the victim
    /// always reaches a decision and can never be locked forever. Checked across a spread of
    /// generated fighters so the DEX/skill lever tuning (which shortens `total`) is exercised too.
    #[test]
    fn no_authored_move_can_lock_its_victim_forever() {
        for seed in 0..64 {
            let f = generate_fighter(seed);
            for m in &f.moves {
                let total = m.timing.total();
                for q in &m.qualities {
                    if let QualityKind::Hitbox(a) = &q.kind {
                        assert!(a.hit.hitstun < total, "seed {seed}: hitstun {} ≥ total {total}", a.hit.hitstun);
                        assert!(a.hit.blockstun < total, "seed {seed}: blockstun {} ≥ total {total}", a.hit.blockstun);
                    }
                }
            }
        }
    }

    /// Every compiled fighter carries the universal Guard (a Block quality requiring no body part).
    #[test]
    fn every_fighter_can_guard() {
        let f = compile(&build(Morphology::Quadruped, vec![], avg())); // even a limbless quadruped
        assert!(f.moves.iter().any(|m| {
            m.requires.is_empty() && m.qualities.iter().any(|q| matches!(q.kind, QualityKind::Block { .. }))
        }));
    }
}
