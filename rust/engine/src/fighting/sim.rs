//! The engine driver (mechanics §1.4). A *pure, headless*, **fully turn-based** simulation: it orders
//! actions on a shared tick counter, resolves contacts when an attack window is live (a 3D hitbox vs
//! the defender's hurtboxes), and **pauses** whenever an entity must choose — exactly the `FightState`
//! loop (`advance()` ↔ `Advancing`, a returned `Decision` ↔ `AwaitInput`).
//!
//! Every magnitude it applies is read from the **authored move** (parry freeze, blockstun, counter
//! bonus, knockdown duration, throw-tech recover); the engine has no combat constants. Moves are also
//! **morphology-gated** — a fighter can only use a move whose required body parts it has.
//!
//! Geometry uses `bevy_math` AABBs (f32); see [`super::space`]. Logic is index-ordered for a
//! reproducible run on a given platform.

use super::entity::*;
use super::frame::*;
use super::regime::Regime;
use super::resolver::{classify_contact, ContactResult};
use super::space::{overlaps, place, Box3};

pub type Tick = u32;

/// A committed choice at a decision point.
#[derive(Clone, Copy, Debug)]
pub enum Action {
    Wait,
    Use(MoveId),
}

/// Who the engine is waiting on, and under which regime.
#[derive(Clone, Debug)]
pub enum Decision {
    /// All listed entities are free at once → commit hidden, simultaneously (the mind-read).
    Neutral(Vec<EntityId>),
    /// One entity is free while ≥1 opponent is locked → it chooses with full information.
    Pressure(EntityId),
}
impl Decision {
    pub fn regime(&self) -> Regime {
        match self {
            Decision::Neutral(_) => Regime::Neutral,
            Decision::Pressure(_) => Regime::Pressure,
        }
    }
}

/// Why the fight ended.
#[derive(Clone, Copy, Debug)]
pub enum EndReason {
    Victory(SideId),
    /// Every remaining side eliminated on the same tick (double-KO) — the no-timer tie (fsm.md).
    Draw,
    /// The optional `max_ticks` safety bound was hit (not a game timer).
    TickCap,
}

/// The result of pumping the sim.
#[derive(Clone, Debug)]
pub enum Outcome {
    Decision(Decision),
    Ended(EndReason),
}

/// How a surviving defender reacts to a connecting hit (resolved from authored move data).
enum PostHit {
    Hitstun(Tick),
    Airborne(Tick),
    Down(Tick),
}

/// A fully-resolved mutation to apply this tick — all magnitudes already read off the authored move.
enum Effect {
    Hit { target: EntityId, amount: Health, knockback: f32, att_facing: i8, survive: PostHit },
    Armor { target: EntityId, amount: Health },
    Block { target: EntityId, ticks: Tick },
    Parry { attacker: EntityId, defender: EntityId, freeze: Tick, recover: Tick },
    ThrowTech { a: EntityId, b: EntityId, recover: Tick },
}

/// One fight in progress.
pub struct Sim {
    pub tick: Tick,
    pub entities: Vec<Entity>,
    pub moves: MoveTable,
    /// Optional safety termination bound (default `None`). NOT a game timer — see [`super::config`].
    pub max_ticks: Option<Tick>,
}

impl Sim {
    pub fn new(entities: Vec<Entity>, moves: MoveTable) -> Self {
        Self { tick: 0, entities, moves, max_ticks: None }
    }

    /// Can entity `id` use `move_id`? True iff the move exists and the entity's body meets its
    /// morphology requirements. Callers/AI should filter their options through this.
    pub fn can_use(&self, id: EntityId, move_id: MoveId) -> bool {
        self.moves
            .get(&move_id)
            .map(|p| self.entities[id].body.satisfies(&p.requires))
            .unwrap_or(false)
    }

    /// Simulate **at most one tick** of work, exposing the in-between frames a driver needs to
    /// *animate* a move (startup → active → recovery). Returns `Some(outcome)` when the sim is
    /// already parked at a decision or an end — *no* tick ran, so `commit`/stop — or `None` when
    /// exactly one tick advanced and the caller should step again. [`Sim::advance`] is just this
    /// looped to the next decision; the two are behaviourally identical at decision granularity.
    pub fn step(&mut self) -> Option<Outcome> {
        if let Some(end) = self.end_condition() {
            return Some(Outcome::Ended(end));
        }
        if let Some(d) = self.pending_decision() {
            return Some(Outcome::Decision(d));
        }
        if let Some(cap) = self.max_ticks {
            if self.tick >= cap {
                return Some(Outcome::Ended(EndReason::TickCap));
            }
        }
        self.tick += 1;
        self.apply_motion();
        self.resolve_tick();
        self.complete_moves();
        self.recover_from_stun();
        None
    }

    /// Run until the next decision point or an end condition. Idempotent while a decision is pending
    /// (call [`Sim::commit`] to resolve it before advancing again). Equivalent to looping [`step`].
    pub fn advance(&mut self) -> Outcome {
        loop {
            if let Some(outcome) = self.step() {
                return outcome;
            }
        }
    }

    /// Apply the chosen actions for the entities named in the current `Decision`.
    pub fn commit(&mut self, choices: &[(EntityId, Action)]) {
        for &(id, action) in choices {
            match action {
                // yield: let one tick pass so the clock can progress.
                Action::Wait => self.entities[id].ready_tick = self.tick + 1,
                Action::Use(move_id) => self.start_move(id, move_id),
            }
        }
    }

    fn start_move(&mut self, id: EntityId, move_id: MoveId) {
        // Morphology gate: an unusable move (missing parts / unknown id) is a no-op yield.
        let (applicable, total) = match self.moves.get(&move_id) {
            Some(p) => (self.entities[id].body.satisfies(&p.requires), p.timing.total()),
            None => (false, 0),
        };
        if !applicable {
            self.entities[id].ready_tick = self.tick + 1;
            return;
        }
        let e = &mut self.entities[id];
        e.action = Some(MoveInstance {
            move_id,
            start_tick: self.tick,
            armor_used: 0,
            connected: false,
            contact: None,
        });
        e.reaction = Reaction::Neutral;
        e.ready_tick = self.tick + total;
    }

    // ---- regime / decisions -------------------------------------------------

    /// Who the engine is currently waiting on (the same `Decision` a pending `advance` returns).
    /// Public so a driver can resolve each ready actor's action without re-advancing.
    pub fn pending_decision(&self) -> Option<Decision> {
        let now = self.tick;
        let contenders: Vec<EntityId> =
            (0..self.entities.len()).filter(|&i| self.entities[i].is_alive()).collect();
        let actionable: Vec<EntityId> =
            contenders.iter().copied().filter(|&i| self.entities[i].is_actionable(now)).collect();
        if actionable.is_empty() {
            return None;
        }
        let everyone_free = actionable.len() == contenders.len() && contenders.len() >= 2;
        if everyone_free {
            Some(Decision::Neutral(actionable))
        } else {
            let actor = *actionable
                .iter()
                .min_by_key(|&&i| self.entities[i].ready_tick)
                .unwrap();
            Some(Decision::Pressure(actor))
        }
    }

    fn end_condition(&self) -> Option<EndReason> {
        let mut sides: Vec<u8> =
            self.entities.iter().filter(|e| e.is_alive()).map(|e| e.side.0).collect();
        sides.sort_unstable();
        sides.dedup();
        match sides.len() {
            0 => Some(EndReason::Draw),
            1 => Some(EndReason::Victory(SideId(sides[0]))),
            _ => None,
        }
    }

    // ---- per-tick simulation ------------------------------------------------

    /// Apply each movement move's one-shot reposition at its first active tick (spec §1.3).
    fn apply_motion(&mut self) {
        let now = self.tick;
        for i in 0..self.entities.len() {
            let (mid, start) = match &self.entities[i].action {
                Some(m) => (m.move_id, m.start_tick),
                None => continue,
            };
            let Some(p) = self.moves.get(&mid) else { continue };
            let Some(m) = p.motion else { continue };
            if now - start != p.timing.startup {
                continue; // only on the first active tick
            }
            let e = &mut self.entities[i];
            let f = e.facing as f32;
            e.pos.x += f * m.delta.x;
            e.pos.y += m.delta.y;
            e.pos.z += m.delta.z;
        }
    }

    fn resolve_tick(&mut self) {
        let now = self.tick;
        let n = self.entities.len();
        let mut effects: Vec<Effect> = Vec::new();
        let mut connected: Vec<EntityId> = Vec::new();

        // Phase 1 (immutable): read live qualities, test 3D overlap, classify, resolve magnitudes.
        for ai in 0..n {
            let att = &self.entities[ai];
            if !att.is_alive() {
                continue;
            }
            let Some(inst) = &att.action else { continue };
            if inst.connected {
                continue;
            }
            let Some(ap) = self.moves.get(&inst.move_id) else { continue };
            let ae = now - inst.start_tick;
            let Some(attack) = ap.active_hitbox(ae) else { continue }; // no live hitbox → no contact
            let att_facing = att.facing;

            for di in 0..n {
                if di == ai {
                    continue;
                }
                let def = &self.entities[di];
                if !def.is_alive() || def.side == att.side {
                    continue;
                }
                let (dp, de) = match &def.action {
                    Some(dm) => (self.moves.get(&dm.move_id), Some(now - dm.start_tick)),
                    None => (None, None),
                };
                // type — invuln of the matching category wins outright.
                let def_invuln = dp.and_then(|p| de.and_then(|e| p.active_invuln(e)));
                if let Some(it) = def_invuln {
                    if it == InvulnType::All || it == attack.kind.as_invuln() {
                        continue;
                    }
                }
                // 3D overlap: the attacker's hitbox vs the defender's hurtboxes.
                let hb = hitbox_world(att, attack, def);
                if !overlaps(&hb, &def.hurtboxes()) {
                    continue;
                }

                let def_block = dp.and_then(|p| de.and_then(|e| p.active_block(e)));
                let def_parry = dp.and_then(|p| de.and_then(|e| p.active_parry(e)));
                let def_armor = dp.and_then(|p| de.and_then(|e| p.active_armor(e)));
                let def_throwing = dp.zip(de).map(|(p, e)| p.is_throwing(e)).unwrap_or(false);
                let def_cv = dp.zip(de).map(|(p, e)| p.counter_vulnerable(e)).unwrap_or(false);

                let result =
                    classify_contact(attack, def, def_block, def_parry, def_armor, def_throwing, def_cv);

                match result {
                    ContactResult::Whiff => {}
                    ContactResult::Parried => {
                        let (freeze, recover) = def_parry.unwrap();
                        effects.push(Effect::Parry { attacker: ai, defender: di, freeze, recover });
                    }
                    ContactResult::ThrowTech => {
                        effects.push(Effect::ThrowTech { a: ai, b: di, recover: attack.tech_recover });
                    }
                    ContactResult::Blocked => {
                        effects.push(Effect::Block { target: di, ticks: attack.hit.blockstun });
                    }
                    ContactResult::Armored => {
                        let (_, num, den) = def_armor.unwrap();
                        let scaled = attack.hit.damage * num / den.max(1);
                        effects.push(Effect::Armor { target: di, amount: scaled });
                    }
                    ContactResult::Thrown => {
                        effects.push(resolve_hit(di, att_facing, attack, false));
                    }
                    ContactResult::Hit { counter } => {
                        effects.push(resolve_hit(di, att_facing, attack, counter));
                    }
                }
                connected.push(ai);
                break; // one contact per attacker per tick
            }
        }

        // Phase 2 (mutable): apply in deterministic order.
        for e in effects {
            self.apply_effect(e);
        }
        for ai in connected {
            if let Some(inst) = &mut self.entities[ai].action {
                inst.connected = true;
            }
        }
    }

    fn apply_effect(&mut self, e: Effect) {
        let now = self.tick;
        match e {
            Effect::Hit { target, amount, knockback, att_facing, survive } => {
                let d = &mut self.entities[target];
                d.health = d.health.saturating_sub(amount);
                d.pos.x += if att_facing >= 0 { knockback } else { -knockback };
                d.action = None; // a clean hit interrupts whatever the defender was doing
                if d.health == 0 {
                    d.reaction = Reaction::KO;
                    d.ready_tick = Tick::MAX;
                } else {
                    match survive {
                        PostHit::Hitstun(t) => {
                            d.reaction = Reaction::Hitstun;
                            d.ready_tick = now + t;
                        }
                        PostHit::Airborne(t) => {
                            d.reaction = Reaction::Airborne;
                            d.ready_tick = now + t;
                        }
                        PostHit::Down(t) => {
                            d.reaction = Reaction::Down;
                            d.ready_tick = now + t;
                        }
                    }
                }
            }
            Effect::Armor { target, amount } => {
                let d = &mut self.entities[target];
                if let Some(m) = &mut d.action {
                    m.armor_used += 1;
                }
                d.health = d.health.saturating_sub(amount);
                if d.health == 0 {
                    d.action = None;
                    d.reaction = Reaction::KO;
                    d.ready_tick = Tick::MAX;
                }
            }
            Effect::Block { target, ticks } => {
                let d = &mut self.entities[target];
                d.action = None;
                d.reaction = Reaction::Blockstun;
                d.ready_tick = now + ticks;
            }
            Effect::Parry { attacker, defender, freeze, recover } => {
                let a = &mut self.entities[attacker];
                a.action = None;
                a.reaction = Reaction::Parried;
                a.ready_tick = now + freeze;
                let d = &mut self.entities[defender];
                d.action = None;
                d.reaction = Reaction::Neutral;
                d.ready_tick = now + recover;
            }
            Effect::ThrowTech { a, b, recover } => {
                for x in [a, b] {
                    let e = &mut self.entities[x];
                    e.action = None;
                    e.reaction = Reaction::Neutral;
                    e.ready_tick = now + recover;
                }
            }
        }
    }

    /// Return any fighter whose **stun has worn off** to `Neutral`. A hit/block/parry leaves the
    /// victim with `action = None` and a transient `reaction` (Hitstun/Blockstun/Airborne/Down/…)
    /// plus a `ready_tick`. When that tick arrives nothing else clears the reaction — `complete_moves`
    /// only resets fighters who are mid-*move*, and skips anyone whose `action` is `None` — so without
    /// this the fighter stays flagged stunned forever and [`Entity::is_actionable`] (which requires
    /// `Neutral`) locks it out of every future decision after its first contact. This is the missing
    /// edge of the reaction state machine: stun expiry → `Neutral`. Runs after `resolve_tick`, so a
    /// fighter re-hit on its wake-up tick stays stunned (the new stun's `ready_tick` is still ahead).
    fn recover_from_stun(&mut self) {
        let now = self.tick;
        for e in &mut self.entities {
            if e.is_alive()
                && e.action.is_none()
                && e.ready_tick <= now
                && !matches!(e.reaction, Reaction::Neutral | Reaction::KO)
            {
                e.reaction = Reaction::Neutral;
            }
        }
    }

    /// Retire moves that have run their full course → back to `Neutral`.
    fn complete_moves(&mut self) {
        let now = self.tick;
        for i in 0..self.entities.len() {
            let (mid, start) = match &self.entities[i].action {
                Some(m) => (m.move_id, m.start_tick),
                None => continue,
            };
            let done = match self.moves.get(&mid) {
                Some(p) => now - start >= p.timing.total(),
                None => true,
            };
            if done {
                let e = &mut self.entities[i];
                e.action = None;
                e.reaction = Reaction::Neutral;
            }
        }
    }
}

/// Resolve an attack's hitbox into world space: take its body-sourced (or custom) box, apply the
/// move's local placement, place it by the attacker's pos + facing, then realign on Z per tracking.
fn hitbox_world(att: &Entity, attack: &Attack, def: &Entity) -> Box3 {
    let base = match attack.source {
        HitboxSource::Part(p) => att.body.part(p).unwrap_or(Box3::ZERO),
        HitboxSource::Custom(b) => b,
    };
    let local = Box3 { center: base.center + attack.placement, half: base.half };
    let mut hb = place(local, att.pos, att.facing);
    match attack.tracking {
        Tracking::Homing => hb.center.z = def.pos.z,
        Tracking::Tracking(side) => {
            if ((def.pos.z - hb.center.z).signum() as i8) == side {
                hb.center.z = def.pos.z;
            }
        }
        Tracking::Linear => {}
    }
    hb
}

/// Resolve a connecting strike/throw into a `Hit` effect, reading the attack's authored magnitudes.
fn resolve_hit(di: EntityId, att_facing: i8, attack: &Attack, counter: bool) -> Effect {
    let mut amount = attack.hit.damage;
    let mut hitstun = attack.hit.hitstun;
    if counter {
        if let Some(cb) = attack.counter {
            amount = (amount * cb.dmg_num + cb.dmg_den / 2) / cb.dmg_den.max(1);
            hitstun += cb.hitstun_bonus;
        }
    }
    let survive = if attack.hit.launches {
        PostHit::Airborne(hitstun)
    } else if let Some(d) = attack.hit.knockdown {
        PostHit::Down(d)
    } else {
        PostHit::Hitstun(hitstun)
    };
    Effect::Hit { target: di, amount, knockback: attack.hit.knockback, att_facing, survive }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::space::{BodyPart, Box3, Vec3};

    fn v(x: f32, y: f32, z: f32) -> Vec3 {
        Vec3::new(x, y, z)
    }

    /// A humanoid body: a torso hurtbox and a fist (hurtbox + punch source).
    fn humanoid() -> Body {
        Body {
            parts: vec![
                (BodyPart::Torso, Box3::new(v(0.0, 1.0, 0.0), v(0.4, 0.8, 0.4))),
                (BodyPart::Fist, Box3::new(v(0.4, 1.0, 0.0), v(0.25, 0.25, 0.25))),
            ],
        }
    }
    /// A fistless body (e.g. a serpent) — torso only.
    fn fistless() -> Body {
        Body { parts: vec![(BodyPart::Torso, Box3::new(v(0.0, 1.0, 0.0), v(0.4, 0.8, 0.4)))] }
    }

    fn hit(damage: Health, hitstun: Tick) -> HitEffect {
        HitEffect {
            damage,
            hitstun,
            blockstun: 6,
            chip: 0,
            knockback: 0.0,
            launches: false,
            knockdown: None,
        }
    }

    fn punch_attack(counter: Option<CounterBonus>, tracking: Tracking) -> Attack {
        Attack {
            kind: AttackKind::Strike,
            guard: GuardHeight::Mid,
            blockable: true,
            source: HitboxSource::Part(BodyPart::Fist), // geometry falls out of the body
            placement: v(0.5, 0.0, 0.0),                // reaches forward
            tracking,
            hit: hit(8, 12),
            counter,
            tech_recover: 0,
        }
    }

    /// A punch: requires a Fist; hitbox is the body's fist box, live on its active frames.
    fn punch() -> FrameProfile {
        FrameProfile {
            timing: Timing { startup: 3, active: 2, recovery: 5 },
            qualities: vec![Quality {
                from: 3,
                to: 4,
                kind: QualityKind::Hitbox(punch_attack(None, Tracking::Linear)),
            }],
            motion: None,
            requires: vec![BodyPart::Fist],
        }
    }

    fn ch_punch() -> FrameProfile {
        let mut p = punch();
        if let QualityKind::Hitbox(a) = &mut p.qualities[0].kind {
            a.counter = Some(CounterBonus { dmg_num: 2, dmg_den: 1, hitstun_bonus: 10 });
        }
        p
    }

    fn homing_punch() -> FrameProfile {
        let mut p = punch();
        if let QualityKind::Hitbox(a) = &mut p.qualities[0].kind {
            a.tracking = Tracking::Homing;
        }
        p
    }

    fn slow() -> FrameProfile {
        let mut p = punch();
        p.timing = Timing { startup: 20, active: 2, recovery: 5 };
        if let QualityKind::Hitbox(a) = &mut p.qualities[0].kind {
            a.hit = hit(30, 12);
        }
        p.qualities[0].from = 20;
        p.qualities[0].to = 21;
        p
    }

    fn guard() -> FrameProfile {
        FrameProfile {
            timing: Timing { startup: 0, active: 30, recovery: 5 },
            qualities: vec![Quality {
                from: 0,
                to: 29,
                kind: QualityKind::Block { covers: vec![GuardHeight::High, GuardHeight::Mid] },
            }],
            motion: None,
            requires: vec![], // anyone can guard
        }
    }

    fn fighter(side: u8, body: Body, pos: Vec3, facing: i8) -> Entity {
        Entity {
            side: SideId(side),
            pos,
            facing,
            body,
            health: 100,
            ready_tick: 0,
            action: None,
            reaction: Reaction::Neutral,
        }
    }

    fn sim_with(b: Body, pos_b: Vec3, table: &[(u32, FrameProfile)]) -> Sim {
        let mut moves = MoveTable::new();
        for (id, p) in table {
            moves.insert(MoveId(*id), p.clone());
        }
        let a = fighter(0, humanoid(), v(0.0, 0.0, 0.0), 1);
        let b = fighter(1, b, pos_b, -1);
        let mut sim = Sim::new(vec![a, b], moves);
        sim.max_ticks = Some(80);
        sim
    }

    fn drain(sim: &mut Sim) {
        loop {
            match sim.advance() {
                Outcome::Decision(Decision::Neutral(ids)) => {
                    let c: Vec<_> = ids.iter().map(|&i| (i, Action::Wait)).collect();
                    sim.commit(&c);
                }
                Outcome::Decision(Decision::Pressure(i)) => sim.commit(&[(i, Action::Wait)]),
                Outcome::Ended(_) => break,
            }
        }
    }

    #[test]
    fn punch_connects_in_3d() {
        let mut sim = sim_with(humanoid(), v(1.0, 0.0, 0.0), &[(1, punch())]);
        let _ = sim.advance();
        sim.commit(&[(0, Action::Use(MoveId(1))), (1, Action::Wait)]);
        drain(&mut sim);
        assert_eq!(sim.entities[1].health, 92);
    }

    #[test]
    fn sidestep_dodges_a_linear_punch() {
        // B stands off-axis on Z → the linear fist box never overlaps B's hurtboxes.
        let mut sim = sim_with(humanoid(), v(1.0, 0.0, 2.0), &[(1, punch())]);
        let _ = sim.advance();
        sim.commit(&[(0, Action::Use(MoveId(1))), (1, Action::Wait)]);
        drain(&mut sim);
        assert_eq!(sim.entities[1].health, 100);
    }

    #[test]
    fn homing_punch_beats_the_step() {
        // Same off-axis B, but a HOMING punch realigns on Z → it connects.
        let mut sim = sim_with(humanoid(), v(1.0, 0.0, 2.0), &[(1, homing_punch())]);
        let _ = sim.advance();
        sim.commit(&[(0, Action::Use(MoveId(1))), (1, Action::Wait)]);
        drain(&mut sim);
        assert_eq!(sim.entities[1].health, 92);
    }

    #[test]
    fn counter_bonus_is_authored_on_the_move() {
        let mut sim = sim_with(humanoid(), v(1.0, 0.0, 0.0), &[(1, ch_punch()), (2, slow())]);
        let _ = sim.advance();
        sim.commit(&[(0, Action::Use(MoveId(1))), (1, Action::Use(MoveId(2)))]);
        drain(&mut sim);
        assert_eq!(sim.entities[1].health, 84); // authored ×2 on counter
    }

    #[test]
    fn block_is_a_move_quality() {
        let mut sim = sim_with(humanoid(), v(1.0, 0.0, 0.0), &[(1, punch()), (3, guard())]);
        let _ = sim.advance();
        sim.commit(&[(0, Action::Use(MoveId(1))), (1, Action::Use(MoveId(3)))]);
        drain(&mut sim);
        assert_eq!(sim.entities[1].health, 100);
    }

    #[test]
    fn move_is_gated_by_morphology() {
        // A fistless fighter cannot use a punch (requires Fist).
        let sim = sim_with(fistless(), v(1.0, 0.0, 0.0), &[(1, punch())]);
        assert!(sim.can_use(0, MoveId(1))); // humanoid A has a fist
        assert!(!sim.can_use(1, MoveId(1))); // fistless B does not

        // And committing it is a no-op: B stays at full health, A is untouched.
        let mut sim = sim_with(fistless(), v(1.0, 0.0, 0.0), &[(1, punch())]);
        let _ = sim.advance();
        sim.commit(&[(1, Action::Use(MoveId(1))), (0, Action::Wait)]);
        drain(&mut sim);
        assert_eq!(sim.entities[0].health, 100);
    }

    #[test]
    fn a_struck_fighter_recovers_and_acts_again() {
        // A lands one punch on B, then both idle. Once B's hitstun expires it must return to Neutral
        // and be offered another decision — never permanently locked out (the recovery-edge bug).
        let mut sim = sim_with(humanoid(), v(1.0, 0.0, 0.0), &[(1, punch())]);
        let _ = sim.advance();
        sim.commit(&[(0, Action::Use(MoveId(1))), (1, Action::Wait)]);

        let full = sim.entities[1].health;
        let mut b_was_hit = false;
        let mut b_decided_after_hit = false;
        for _ in 0..200 {
            match sim.advance() {
                Outcome::Decision(d) => {
                    if sim.entities[1].health < full {
                        b_was_hit = true;
                    }
                    let ids = match &d {
                        Decision::Neutral(v) => v.clone(),
                        Decision::Pressure(i) => vec![*i],
                    };
                    if b_was_hit && ids.contains(&1) {
                        b_decided_after_hit = true;
                        break;
                    }
                    let c: Vec<_> = ids.iter().map(|&i| (i, Action::Wait)).collect();
                    sim.commit(&c);
                }
                Outcome::Ended(_) => break,
            }
        }
        assert!(b_was_hit, "setup: B should have taken the punch");
        assert!(b_decided_after_hit, "B was locked out after being hit — never recovered to Neutral");
    }

    #[test]
    fn deterministic() {
        let run = || {
            let mut sim = sim_with(humanoid(), v(1.0, 0.0, 0.0), &[(1, ch_punch()), (2, slow())]);
            let _ = sim.advance();
            sim.commit(&[(0, Action::Use(MoveId(1))), (1, Action::Use(MoveId(2)))]);
            drain(&mut sim);
            (sim.entities[0].health, sim.entities[1].health, sim.tick)
        };
        assert_eq!(run(), run());
    }
}
