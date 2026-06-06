/**
 * config.ts — all locked feature flags and tuning constants in one place (core/, L0).
 *
 * The 12 locked decisions and the spec's defaults are centralized here so a port can read the
 * exact numbers from one file. Values that are genuinely playtest-tuned are marked TUNING; the
 * locked design decisions are marked with their decision number.
 *
 * NOTE: combat multipliers are stored as `Fixed` (16.16) so damage scaling stays integer/
 * deterministic (no float math in gameplay — decision 10). Apply them via fixed.mul then toInt.
 */
import { type Fixed, fromRatio } from "./fixed";

export const CONFIG = {
  /** Locked feature flags (the resolved ⚠️ forks). */
  features: {
    /** Decision 1: armor absorbs strikes only; throws connect through armor. */
    THROWS_BEAT_ARMOR: true,
    /** Decision 3: fixed damage for the prototype — no RNG is wired into damage when false. */
    DAMAGE_VARIANCE: false,
    /** Decision 6: a move is cancelable only from active/recovery unless it sets startupCancelable. */
    STARTUP_CANCELABLE_BY_DEFAULT: false,
  },

  tick: {
    /** Convention: 1 tick = 1 frame at 60 Hz (spec §0.1). Informational; the engine is tick-based. */
    TICKS_PER_SECOND: 60,
  },

  /** Action economy / Tempo (spec §3.5; decisions 4 & 5). */
  ap: {
    /** Decision 5: AP_max = AP_BASE + tempoTier. */
    AP_BASE: 3,
    /**
     * Decision 5: tempoMod = roundHalfUp((dexMod + wisMod)/2); tempoTier = count of thresholds ≤ tempoMod.
     * TUNING — playtest curve. With these thresholds a strong DEX+WIS build (tempoMod ≥ 3) reaches
     * tier 2 → AP_max 5, matching the spec's worked-example "tempo" variant.
     */
    TEMPO_TIER_THRESHOLDS: [1, 3, 5] as const,
    /** spec §3.5.1: ap_refill default = AP_max each time initiative is (re)gained. */
    REFILL_TO_MAX: true,
  },

  /** Combat resolution constants (spec §2.7, §2.8; decision 7). */
  combat: {
    /** spec §2.7: counter-hit damage ×1.25. */
    CH_DAMAGE_MULT: fromRatio(5, 4) as Fixed,
    /** spec §2.7: counter-hit +6 ticks of hitstun. */
    CH_HITSTUN_BONUS: 6,
    /** spec §2.8: juggle damage decay ×0.9 per successive juggle hit (0.9^n). */
    JUGGLE_DAMAGE_DECAY: fromRatio(9, 10) as Fixed,
    /** Decision 7 / spec §2.6: parry refunds a small amount of Focus on success. TUNING. */
    PARRY_FOCUS_REFUND: 1,
    /** Decision 7 / spec §3.5.2: parry refunds AP on success (ON_PARRY +2). TUNING. */
    PARRY_AP_REFUND: 2,
    /** Ticks the attacker is frozen after being parried — "huge plus for defender" (spec §2.6). TUNING. */
    PARRY_FREEZE_TICKS: 30,
    /** Ticks before the successful parrier is actionable (a quick, large advantage). TUNING. */
    PARRY_RECOVER_TICKS: 4,
    /** Ticks both throwers recover after a throw-tech clash (spec §2.6). TUNING. */
    THROW_TECH_RECOVER_TICKS: 8,
    /** Stun ticks of a guard break (a long, fully-punishable stun — spec §2.5). TUNING. */
    GUARD_BREAK_STUN_TICKS: 40,
  },

  /** Combo governors (spec §2.8, §3.4). */
  combo: {
    /** Governor 3: each successive combo hit reduces effective hitstun by this many ticks. TUNING. */
    HITSTUN_DECAY_PER_HIT: 2,
    /** Floor on effective hitstun so a combo hit still connects but eventually goes minus. TUNING. */
    MIN_HITSTUN: 1,
  },

  /** Resource regeneration (spec §3.1). */
  resources: {
    /** Stamina regained per tick while NOT executing a move (spec §3.1). TUNING. */
    STAMINA_REGEN_PER_TICK: 1,
  },

  /**
   * RPG → frame-data curves (spec §4.2, §4.5). Read ONLY by rpg/compiler.ts (the single bridge).
   * WWN-style LOW modifiers, so frame-data swings stay small and the engine stays the star. All TUNING
   * — these are the playtest-tuned "modifier → tick/stat" tables the spec flags as the largest open
   * work item. R-2: each attribute drives exactly ONE major lever (no double-dip). R-3: gates are
   * floors; exceeding a requirement gives a small CAPPED bonus, never runaway scaling.
   */
  rpg: {
    // DEX → speed (−startup on lights/normals), capped (R-3).
    DEX_STARTUP_REDUCTION_PER_MOD: 1,
    DEX_STARTUP_REDUCTION_CAP: 3,
    DEX_ADVANCE_PER_MOD: 1, // +movement advance (fixed-point units applied by the compiler)
    // STR → damage on heavies/throws + armor budget.
    STR_DAMAGE_PER_MOD: 2,
    STR_ARMOR_HITS_PER_MOD: 1,
    STR_ARMOR_HITS_CAP: 2,
    // CON → survivability (HP / Stamina / Poise).
    CON_HP_PER_MOD: 10,
    CON_STAMINA_PER_MOD: 5,
    CON_POISE_PER_MOD: 3,
    // INT → Focus pool.
    INT_FOCUS_PER_MOD: 2,
    // Base pools before attribute contributions.
    BASE_HP: 100,
    BASE_STAMINA: 50,
    BASE_POISE: 30,
    BASE_FOCUS: 10,
    // A move's startup never drops below this after DEX/weapon reductions.
    MIN_STARTUP: 1,
  },
} as const;

export type Config = typeof CONFIG;
