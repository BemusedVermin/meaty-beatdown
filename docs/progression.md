# TICK — Progression Design
### Getting stronger in a world where getting stronger is the trap

*Companions: [`frame_rpg_spec.md`](./frame_rpg_spec.md) §12 (the Build → Fighter compiler this doc
feeds), [`exploration.md`](./exploration.md) (where the inputs come from),
[`the-promise-plot-bible.md`](./the-promise-plot-bible.md) (why the strongest thing you can do is stop).*

---

## 1. The charter: no XP ✅

There are **no experience points and no character levels**. Nothing numerically grows because a
fight ended. Strength comes from five legible, diegetic axes — each one a thing you *did*, not a
bar that filled:

| Axis | What it is | Where it comes from |
|---|---|---|
| **Rank** | Your Ascension tier — the world's measure of you | trials and deeds (§2) |
| **Attributes** | Six trained qualities of body and mind | training under trainers/masters (§3) |
| **Forms** | The martial styles you know, rank 0–4 each | masters' teaching, scraps, practice (§4) |
| **Gear** | Weapons, armor, talismans + affixes | loot, trade, crafting-lite (§5) |
| **Knowledge** | The codex: matchup tiers per style/enemy | fighting, studying, buying intel (§6) |

Every axis terminates in the same place: the **compiler** (spec §12). Progression is, literally,
*recompiling yourself*.

> Design intent: an hour of play makes you stronger because you *learned a Form, won a trial,
> studied a rival, or found a sword* — all narratable sentences. "I'm 3% stronger because
> numbers" is banned. This is also what makes the Tithe reveal (§8) land: every one of those
> sentences turns out to have been feeding something.

## 2. Rank — the Ascension tiers

The world's ladder (bible §2: rank is social standing, political voice, military value).
Nine named tiers ⚠️ (placeholder count; xianxia convention). Rank is advanced **only** through
**trials**: formal duels and deeds administered by masters or factions.

- **Trials are boss fights with stakes**: announced, preparable, one-on-one by tradition
  (variations authored — some trials are gauntlets, some are 2v2 with your companion, some are
  *thrown* for political reasons and you may notice).
- **Rank gates**: Form tiers a master may teach you (§4), attribute training caps (§3), loadout
  breadth (§7), questlines, and who will even talk to you. Political access *is* a progression
  reward (bible §8: "to deal with great entities you must be one").
- **Rank is also bait.** The top of the ladder is the Crossing. The endgame asks what you do
  with a full ladder — climbing it is the plot's central irony, and mechanically we never
  pretend otherwise (see §8).

## 3. Attributes

Six attributes, renamed from the WWN-style array to setting-native terms ⚠️ (names provisional;
mapping fixed). **Exactly one major combat lever each** (balance rule R-2, audited), keeping
modifiers low so the fighting game stays the star:

| Attribute | (was) | The one major lever | Out of combat |
|---|---|---|---|
| **Body** | STR | damage on heavies & throws; armor-hit budgets on armored moves | force, carry |
| **Grace** | DEX | −startup on lights/normals; movement distances | stealth, deck-work |
| **Vigor** | CON | HP / Breath / Guard maxima | endurance |
| **Mind** | INT | Focus economy: gauge max + cancel-cost efficiency | lore, the lantern |
| **Insight** | WIS | the reads lever: parry/guard-point window width; **codex speed** (knowledge tiers come faster, §6) | perception |
| **Spirit** | CHA | the tempo lever: AP_max and AP refill — presence buys actions | sway, command |

- Trained at **trainers** (low caps) and **masters** (rank-capped) for coin and time — training
  is a scene and a transaction, never a minigame (C-QUAR).
- Caps rise with Rank (rule R-3: gates are floors; over-meeting gives small capped bonuses).
- ⚠️ The lever curves (e.g. "Grace +1 = −1 startup on lights, cap −3") are tuning tables owned
  by playtest, per spec §15.

## 4. Forms — the moveset source

A **Form** is a martial style: a moveset family with a shared **cue vocabulary**, signature
mechanics, and a teaching lineage (the plot lives here — Forms are doctrine made muscle).

- **Form rank 0–4**: rank unlocks the Form's moves in tiers (0 = stance + basics … 4 = the
  signature super) and improves authored per-rank deltas on its moves. Taught by masters with
  standing requirements; **Form scraps** (loot) grant single moves without the lineage —
  street-learned, capped at rank 2 ⚠️, a loot-driven shortcut with a glass ceiling.
- **Everyone knows the First Form** (rank 1, free): the universal basics every child is taught
  — and, per the bible, the prophecy encoded as movement. Its ubiquity is a plot fact wearing a
  tutorial's clothes.
- **Form identity = mechanical identity**: e.g. ⚠️ (sample palette, content-team-owned)
  *Iron Mountain* (armor, guard points, slow plus-frames), *Drifting Leaf* (steps, evasion,
  whiff-punish CH tools), *Burning Star* (strings, Heat synergy, chip pressure), *Hollow Reed*
  (parries, throw escapes, tempo theft), *Breaker's Yoke* (command grabs, sandwich play).
- A character knows several Forms; **the loadout (§7) is where styles braid** into a personal
  game plan. Two players with the same Forms should still play like different Tekken mains
  (vision pillar: builds are characters).

## 5. Gear & affixes

Generation rules live in `exploration.md` §6; build rules here:

- **Weapon = spacing identity** (the R-4 triangle: range ↔ speed ↔ damage; no Pareto winner) +
  granted moves. Changing weapons meaningfully changes your neutral.
- **Armor = defense profile trades** (Guard max, weight/juggle behavior, block arc vs. mobility).
- **Talismans = property riders** (the buildcraft spice: "+1 armor hit on stance moves,"
  "Focus +2 on parry," "your backdash gains 2 i-frames").
- **Affixes are budget-priced deltas** (spec §13): loot can make you *different* and better,
  never budget-breaking. Rarity buys more and stranger riders, not law-breaking ones.
- Crafting is deliberately lite ⚠️: re-roll / transfer one affix at port artisans — a coin sink
  and a bad-luck valve, not a system of its own.

## 6. Knowledge — the codex

The fog of war (spec §7) made progression: per-move knowledge tiers, aggregated per Form/enemy.

- **Earned by fighting**: seeing moves resolve (T1), repeated exposure (T2), deep familiarity
  (T3 — break keys, phase readouts, habit-ordered candidate sets). Thresholds ⚠️ tuned per
  enemy rarity; **Insight** accelerates the climb (§3).
- **Bought**: intel drops and port scholars sell tiers for coin (knowledge is loot); masters
  teach matchup courses for standing (knowledge is curriculum); rivals *trade* it (knowledge is
  diplomacy).
- **Never lost**, never random: the codex is a strictly-growing map of the world's martial
  truth — the exploration fantasy and the fighting-game "lab" fantasy are the same feature here,
  expressed as an RPG journal (C-QUAR: consulted in menus as recorded fact, not practiced as a
  minigame).

## 7. The loadout

You equip a **deck** of K moves ⚠️ (K grows with Rank) from everything your Forms, gear, and
scraps grant — plus the universals (guard, movement set, throw, switch-focus, and your Burst).
The loadout screen is the buildcraft heart: it shows authored qualities in RPG language
(reach, speed, what it beats), with full frame data one deliberate tap deeper. Companions'
loadouts are yours to manage too (§9). Combos are *planned* here only in the sense that you
choose your tools; nothing on this screen simulates a fight (C-QUAR).

## 8. The Tithe ✅

The hidden meter (bible §8). Accumulates from fights won, training completed, rank trials
passed — scaled by tier (the harvest prefers refinement). Invisible and inert until **Gate 3**;
then revealed, retroactively graphed over your whole campaign, and coupled (gently, authored)
to the Fog border (`exploration.md` §7). The progression system itself is the delivery
mechanism for the game's thesis: every axis in §1 feeds it **except Knowledge** — learning is
the one hunger the Hollow cannot eat ⚠️ (design intent: the cessation path and the codex are
the two things that grow you without feeding it; keep this exception sacred).

## 9. Companions

Party members progress on the same five axes — no shadow XP. They train where you train (their
coin, your call), hold their own Forms (story-fixed cores + flexible periphery), carry their own
codices ⚠️ (or share yours — tuning call for UI sanity), and their loadouts are player-managed.
Their *Rank*, however, is theirs: trials are personal, and a companion's climb (or refusal to
climb) is story material the systems must not flatten.

## 10. Currencies, summarized

| Currency | Sink | Source |
|---|---|---|
| **Coin** | gear, intel, training, crafting-lite, repairs | loot, trade, bounties |
| **Standing** (per faction) | masters' teaching, questlines, political doors | deeds, trials, choices |
| **Time** (watches) | travel, training, world clocks | spent, never bought back |
| **Anchor charge** | deep-fog travel | masters' islands, story upgrades |

No currency converts into another at a menu; each is earned in its own register. (Standing
versus coin is the political-vs-mercantile texture; time is the one everyone pays.)
