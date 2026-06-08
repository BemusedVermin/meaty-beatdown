//! # `render` — minimal 2D presentation (so the prototype is playable)
//!
//! View-only: it reads engine + driver state and never mutates the simulation. A top-down hexcrawl
//! drawn with sprites (tiles + POIs) and gizmos (encounters + the ship); a combat overlay drawn with
//! gizmos (stick-figure hurtboxes); a one-line text HUD; and the session bootstrap that gets us from
//! boot into the overworld.
//!
//! **Controls**
//! - Overworld: `W`/`↑` sail forward, `A`/`D` (or `←`/`→`) turn the ship. Sail onto a red marker to fight.
//! - Combat: number keys `1..9` use your moves (shown in the HUD), `Space` waits.
//! - Outcome screen: `Enter` / `Space` to continue.

use bevy::math::Isometry2d;
use bevy::prelude::*;
use engine::exploration::{Hex, Poi, Terrain};
use engine::fighting::{
    phase_at, BodyPart, Entity, FrameProfile, HitboxSource, Phase, QualityKind, Reaction, Sim, Vec3,
};

use crate::combat::ActiveFight;
use crate::exploration::{Overworld, Voyage};
use crate::state::{AppState, CombatState, GameState};

const HEX_SIZE: f32 = 18.0;
const COMBAT_SCALE: f32 = 110.0; // engine world units → screen pixels in the fight view
const SPREAD: f32 = 160.0; // screen-x each fighter is drawn from centre (so they read as far apart)

/// Marks a static overworld sprite (tile / POI) so it can be hidden while the combat overlay is up.
#[derive(Component)]
struct OverworldSprite;

/// The single HUD text line.
#[derive(Component)]
struct HudText;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.04, 0.07, 0.13)))
            .add_systems(Startup, setup)
            .add_systems(Update, boot.run_if(in_state(AppState::Logos)))
            .add_systems(OnEnter(AppState::InSession), enter_session)
            .add_systems(
                Update,
                (spawn_overworld, draw_overworld_markers, follow_player, toggle_overworld)
                    .run_if(in_state(GameState::Exploration)),
            )
            .add_systems(Update, snap_camera_for_combat)
            .add_systems(Update, draw_combat.run_if(resource_exists::<ActiveFight>))
            .add_systems(Update, update_hud);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        Text::new(""),
        TextFont { font_size: 16.0, ..default() },
        TextColor(Color::WHITE),
        Node { position_type: PositionType::Absolute, left: Val::Px(10.0), top: Val::Px(8.0), ..default() },
        HudText,
    ));
}

/// Boot straight into a session (no menu yet) so there's something to play.
fn boot(mut next_app: ResMut<NextState<AppState>>) {
    next_app.set(AppState::InSession);
}

/// A session began → drop into the overworld.
fn enter_session(mut next_game: ResMut<NextState<GameState>>) {
    next_game.set(GameState::Exploration);
}

// ── Overworld view ───────────────────────────────────────────────────────────────────────────────

/// Spawn the static tile (hexagon meshes) + POI sprites once, after the world is generated.
fn spawn_overworld(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    world: Option<Res<Overworld>>,
    existing: Query<(), With<OverworldSprite>>,
) {
    let Some(world) = world else { return };
    if !existing.is_empty() {
        return; // already spawned
    }
    // One shared pointy-top hexagon mesh; one material per terrain (reused across all tiles).
    let hex = meshes.add(RegularPolygon::new(HEX_SIZE, 6));
    let m_deep = materials.add(terrain_color(Terrain::Deep));
    let m_shallows = materials.add(terrain_color(Terrain::Shallows));
    let m_reef = materials.add(terrain_color(Terrain::Reef));
    let m_storm = materials.add(terrain_color(Terrain::Storm));
    let m_island = materials.add(terrain_color(Terrain::Island));

    for (h, tile) in world.0.tiles.iter() {
        let p = hex_to_px(*h);
        let mat = match tile.terrain {
            Terrain::Deep => m_deep.clone(),
            Terrain::Shallows => m_shallows.clone(),
            Terrain::Reef => m_reef.clone(),
            Terrain::Storm => m_storm.clone(),
            Terrain::Island => m_island.clone(),
        };
        commands.spawn((
            Mesh2d(hex.clone()),
            MeshMaterial2d(mat),
            Transform::from_xyz(p.x, p.y, 0.0),
            OverworldSprite,
        ));
    }
    for (h, poi) in world.0.pois.iter() {
        let p = hex_to_px(*h);
        commands.spawn((
            Sprite::from_color(poi_color(*poi), Vec2::splat(HEX_SIZE * 0.8)),
            Transform::from_xyz(p.x, p.y, 1.0),
            OverworldSprite,
        ));
    }
}

/// Draw the dynamic markers (encounters + the ship) each frame, so they reflect the live state.
fn draw_overworld_markers(
    mut gizmos: Gizmos,
    combat: Res<State<CombatState>>,
    world: Option<Res<Overworld>>,
    voyage: Option<Res<Voyage>>,
) {
    if *combat.get() != CombatState::Dormant {
        return;
    }
    if let Some(world) = world {
        for hex in world.0.encounters.keys() {
            gizmos.circle_2d(iso(hex_to_px(*hex)), HEX_SIZE * 0.35, Color::srgb(0.95, 0.25, 0.25));
        }
    }
    if let Some(v) = voyage {
        let ship = hex_to_px(v.party.pos);
        let ahead = hex_to_px(v.party.pos.all_neighbors()[v.heading as usize]);
        let dir = (ahead - ship).normalize_or_zero();
        gizmos.circle_2d(iso(ship), HEX_SIZE * 0.32, Color::srgb(1.0, 0.9, 0.3));
        gizmos.arrow_2d(ship, ship + dir * HEX_SIZE * 1.1, Color::srgb(1.0, 0.95, 0.55));
    }
}

/// Keep the camera centred on the ship while roaming.
fn follow_player(
    combat: Res<State<CombatState>>,
    voyage: Option<Res<Voyage>>,
    mut cam: Query<&mut Transform, With<Camera2d>>,
) {
    if *combat.get() != CombatState::Dormant {
        return;
    }
    let (Some(v), Ok(mut t)) = (voyage, cam.single_mut()) else { return };
    let p = hex_to_px(v.party.pos);
    t.translation.x = p.x;
    t.translation.y = p.y;
}

/// Hide the overworld sprites while the combat overlay is up.
fn toggle_overworld(combat: Res<State<CombatState>>, mut q: Query<&mut Visibility, With<OverworldSprite>>) {
    let want = if *combat.get() == CombatState::Dormant { Visibility::Visible } else { Visibility::Hidden };
    for mut v in &mut q {
        *v = want;
    }
}

// ── Combat view ──────────────────────────────────────────────────────────────────────────────────

/// Centre the camera on the arena whenever a fight (or its outcome screen) is up.
fn snap_camera_for_combat(combat: Res<State<CombatState>>, mut cam: Query<&mut Transform, With<Camera2d>>) {
    if *combat.get() == CombatState::Dormant {
        return;
    }
    if let Ok(mut t) = cam.single_mut() {
        t.translation.x = 0.0;
        t.translation.y = 0.85 * COMBAT_SCALE;
    }
}

/// Draw the fight: a perspective stage, then each fighter as an **animated stick figure** at a fixed
/// left/right position (so they read as clearly apart, even though the sim keeps them in melee range).
fn draw_combat(mut gizmos: Gizmos, fight: Res<ActiveFight>) {
    draw_stage(&mut gizmos);
    for e in &fight.0.entities {
        draw_fighter(&mut gizmos, &fight.0, e);
    }
}

/// How a fighter is posed *this tick*, derived from its in-flight move + reaction state. This is what
/// makes the figures move: a whole-body lean (lunge in on a strike, recoil on a hit) and an extension
/// of the one striking limb through the move's startup → active → recovery.
struct Pose {
    /// Local +x lean of the whole figure toward the opponent (negative = recoil away).
    lean: f32,
    /// The drawn joint the active move drives, and how far it reaches (local space).
    strike: Option<(BodyPart, Vec3)>,
    /// Tuck a guarding arm up by the head.
    guarding: bool,
    /// Draw collapsed (KO).
    downed: bool,
}

/// Read the entity's authoritative state into a drawable [`Pose`].
fn pose_of(sim: &Sim, e: &Entity) -> Pose {
    if !e.is_alive() {
        return Pose { lean: -0.06, strike: None, guarding: false, downed: true };
    }
    // Reaction lean while not mid-move (a struck fighter rocks back).
    let react_lean = match e.reaction {
        Reaction::Hitstun | Reaction::Airborne => -0.18,
        Reaction::Down => -0.26,
        Reaction::Blockstun => -0.08,
        _ => 0.0,
    };
    let (Some(inst), Some(p)) = (
        e.action.as_ref(),
        e.action.as_ref().and_then(|i| sim.moves.get(&i.move_id)),
    ) else {
        return Pose { lean: react_lean, strike: None, guarding: false, downed: false };
    };
    let elapsed = sim.tick.saturating_sub(inst.start_tick);

    // A guard stance: raise the arm, lean back a touch.
    if p.qualities.iter().any(|q| matches!(q.kind, QualityKind::Block { .. })) {
        return Pose { lean: -0.05, strike: None, guarding: true, downed: false };
    }

    // A strike: find the hitbox's source + forward reach, scaled by the move phase.
    let Some((source, placement)) = p.qualities.iter().find_map(|q| match &q.kind {
        QualityKind::Hitbox(a) => Some((a.source, a.placement)),
        _ => None,
    }) else {
        return Pose { lean: react_lean, strike: None, guarding: false, downed: false };
    };
    let t = &p.timing;
    let ext = match phase_at(elapsed, t) {
        Phase::Startup => 0.30 * (elapsed as f32 / t.startup.max(1) as f32), // wind up
        Phase::Active => 1.0,                                                // full thrust
        Phase::Recovery => {
            let into = elapsed.saturating_sub(t.startup + t.active) as f32;
            (1.0 - into / t.recovery.max(1) as f32).max(0.0) // retract
        }
        Phase::Done => 0.0,
    };
    Pose {
        lean: 0.14 * ext,
        strike: Some((strike_limb(source), placement * ext)),
        guarding: false,
        downed: false,
    }
}

/// Map a hitbox source onto the drawn joint that should visibly extend (claws/weapons swing from the
/// hand; a bite lunges the head; a foot/knee kicks).
fn strike_limb(src: HitboxSource) -> BodyPart {
    match src {
        HitboxSource::Part(BodyPart::Foot | BodyPart::Knee) => BodyPart::Foot,
        HitboxSource::Part(BodyPart::Fangs) => BodyPart::Head,
        HitboxSource::Part(BodyPart::Fist | BodyPart::Claws | BodyPart::Elbow) => BodyPart::Fist,
        HitboxSource::Part(_) => BodyPart::Torso,
        HitboxSource::Custom(_) => BodyPart::Fist,
    }
}

/// Draw one fighter as a posed stick figure (spine + head + arm + leg).
fn draw_fighter(gizmos: &mut Gizmos, sim: &Sim, e: &Entity) {
    let base = if e.side.0 == 0 { -SPREAD } else { SPREAD };
    let color = match (e.side.0, e.is_alive()) {
        (_, false) => Color::srgb(0.45, 0.45, 0.45),
        (0, _) => Color::srgb(0.40, 0.70, 1.0),
        _ => Color::srgb(1.0, 0.50, 0.40),
    };
    let pose = pose_of(sim, e);
    // Project a body part → screen, applying the whole-body lean + this limb's reach (if it strikes).
    let at = |bp: BodyPart| -> Option<Vec2> {
        let b = e.body.part(bp)?;
        let reach = match pose.strike {
            Some((limb, r)) if limb == bp => r,
            _ => Vec3::ZERO,
        };
        Some(local_to_screen(b.center + Vec3::new(pose.lean, 0.0, 0.0) + reach, base, e.facing))
    };
    let (head, torso, legs, foot) =
        (at(BodyPart::Head), at(BodyPart::Torso), at(BodyPart::Legs), at(BodyPart::Foot));
    // Guard tucks the fist up between head and torso; otherwise it follows any punch reach.
    let fist = if pose.guarding {
        match (head, torso) {
            (Some(h), Some(t)) => Some(h.lerp(t, 0.35) + Vec2::new(0.0, 3.0)),
            _ => at(BodyPart::Fist),
        }
    } else {
        at(BodyPart::Fist)
    };

    if pose.downed {
        // Collapsed: a slumped blob near the floor instead of a standing figure.
        if let (Some(t), Some(l)) = (torso, legs) {
            gizmos.line_2d(t, l, color);
            gizmos.circle_2d(iso(t), 10.0, color);
        }
        return;
    }
    if let (Some(a), Some(b)) = (legs, torso) {
        gizmos.line_2d(a, b, color);
    }
    if let (Some(a), Some(b)) = (torso, head) {
        gizmos.line_2d(a, b, color);
    }
    if let Some(h) = head {
        gizmos.circle_2d(iso(h), 13.0, color);
    }
    if let (Some(a), Some(b)) = (torso, fist) {
        gizmos.line_2d(a, b, color); // arm
    }
    if let (Some(a), Some(b)) = (legs, foot) {
        gizmos.line_2d(a, b, color); // leg
    }
}

/// A fighter-local point (engine X right / Y up, mirrored by facing) → screen, offset to `base_x`.
fn local_to_screen(local: Vec3, base_x: f32, facing: i8) -> Vec2 {
    Vec2::new(base_x + facing as f32 * local.x * COMBAT_SCALE, local.y * COMBAT_SCALE)
}

/// A cheap perspective floor — rails converging to a vanishing point + narrowing depth lines.
fn draw_stage(gizmos: &mut Gizmos) {
    let col = Color::srgb(0.16, 0.16, 0.22);
    let near_y = -0.15 * COMBAT_SCALE;
    let horizon = 1.7 * COMBAT_SCALE;
    let vanish = Vec2::new(0.0, horizon);
    for i in -5..=5 {
        let x = i as f32 * 0.85 * COMBAT_SCALE;
        gizmos.line_2d(Vec2::new(x, near_y), vanish, col);
    }
    for j in 1..=6 {
        let t = j as f32 / 7.0;
        let y = near_y + t * (horizon - near_y);
        let w = 4.3 * COMBAT_SCALE * (1.0 - t);
        gizmos.line_2d(Vec2::new(-w, y), Vec2::new(w, y), col);
    }
}

// ── HUD ──────────────────────────────────────────────────────────────────────────────────────────

fn update_hud(
    combat: Res<State<CombatState>>,
    fight: Option<Res<ActiveFight>>,
    mut hud: Query<&mut Text, With<HudText>>,
) {
    let Ok(mut text) = hud.single_mut() else { return };
    text.0 = match combat.get() {
        CombatState::Dormant => {
            "Overworld — W: sail forward,  A/D: turn the ship.  Sail onto a red marker to fight."
                .to_string()
        }
        CombatState::Victory => "VICTORY!   press Enter".to_string(),
        CombatState::Defeat => "DEFEAT.   press Enter".to_string(),
        CombatState::Escape => "No contest.   press Enter".to_string(),
        _ => match &fight {
            Some(f) => combat_hud(&f.0),
            None => "Preparing the fight…".to_string(),
        },
    };
}

/// HP readout + the player's move keys.
fn combat_hud(sim: &Sim) -> String {
    let mut line = String::from("FIGHT!  ");
    for e in &sim.entities {
        let who = if e.side.0 == 0 { "You" } else { "Foe" };
        line += &format!("{who}: {} HP   ", e.health);
    }
    let mut moves: Vec<_> = sim.moves.keys().copied().filter(|&m| sim.can_use(0, m)).collect();
    moves.sort_by_key(|m| m.0);
    line += "\nMoves: ";
    for (slot, m) in moves.iter().enumerate() {
        line += &format!("[{}] {}   ", slot + 1, move_label(&sim.moves[m]));
    }
    line += "| [Space] wait";
    line
}

/// A short label for a move, inferred from its first hitbox's source.
fn move_label(p: &FrameProfile) -> &'static str {
    match p.qualities.first().map(|q| &q.kind) {
        Some(QualityKind::Hitbox(a)) => match a.source {
            HitboxSource::Part(BodyPart::Fist) => "Punch",
            HitboxSource::Part(BodyPart::Foot) => "Kick",
            HitboxSource::Part(BodyPart::Fangs) => "Bite",
            HitboxSource::Part(BodyPart::Claws) => "Claw",
            HitboxSource::Part(_) => "Strike",
            HitboxSource::Custom(_) => "Weapon",
        },
        Some(QualityKind::Block { .. }) => "Guard",
        _ => "Move",
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────────────────────────

fn iso(p: Vec2) -> Isometry2d {
    Isometry2d::from_translation(p)
}

/// Axial hex → screen pixel (pointy-top).
fn hex_to_px(h: Hex) -> Vec2 {
    let x = HEX_SIZE * 1.732_050_8 * (h.x as f32 + h.y as f32 / 2.0);
    let y = HEX_SIZE * 1.5 * h.y as f32;
    Vec2::new(x, y)
}

fn terrain_color(t: Terrain) -> Color {
    match t {
        Terrain::Deep => Color::srgb(0.06, 0.14, 0.30),
        Terrain::Shallows => Color::srgb(0.10, 0.32, 0.45),
        Terrain::Reef => Color::srgb(0.32, 0.28, 0.16),
        Terrain::Storm => Color::srgb(0.22, 0.22, 0.28),
        Terrain::Island => Color::srgb(0.22, 0.46, 0.22),
    }
}

fn poi_color(p: Poi) -> Color {
    match p {
        Poi::Port { .. } => Color::srgb(0.85, 0.62, 0.32),
        Poi::SectHall { .. } => Color::srgb(0.62, 0.42, 0.85),
        Poi::MasterSeat { .. } => Color::srgb(0.96, 0.86, 0.32),
        Poi::Ruin => Color::srgb(0.55, 0.55, 0.55),
        Poi::DrownedCamp => Color::srgb(0.72, 0.72, 0.56),
        Poi::Threshold => Color::srgb(0.95, 0.95, 0.98),
    }
}
