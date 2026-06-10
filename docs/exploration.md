# TICK — Exploration Design
### The fog-eaten hexcrawl

*Companion docs: [`the-promise-plot-bible.md`](./the-promise-plot-bible.md) (why the world is like
this), [`progression.md`](./progression.md) (what you bring home), [`frame_rpg_spec.md`](./frame_rpg_spec.md)
(what happens when you engage an encounter), [`fsm.md`](./fsm.md) (the state machinery).*

---

## 1. The fantasy

You captain a small ship across a world that is mostly **Fog** — not weather, but *unmaking*.
Where the Fog has passed there is simply less world. What remains are **islands of reality**,
each one anchored — most of them literally — by a **master**: a high Ascendant whose refined
existence keeps the territory real. Between the islands you sail lanes of grey, lantern burning,
picking your fights from what you can see coming, and coming home with frame data in a sack.

The vibe is Pirates-of-the-Caribbean by way of a xianxia apocalypse: free navigation, visible
trouble, ports with personalities, and a horizon that is being quietly deleted.

**Pillars** (and the charter they answer to):

- **You see everything that can hurt you.** No random encounters, ever. Threats are tokens on
  the map; engaging is a choice (or a failure of seamanship, never a dice roll).
- **The world has a clock, not a leash.** The Fog advances on story beats and Crossings whether
  or not you participate (the bible's "doing nothing is a real input"). Nothing waits — but
  nothing chases you down a corridor either.
- **Exploration is an RPG, full stop** (charter C-QUAR). No frame data, no drills, no combat
  previews out here. The overworld's verbs are: sail, dock, talk, trade, train, delve, decide.
- **Loot is the pull, fights are the proof** — Diablo/Borderlands reward logic on a
  fighting-game chassis: the chest contains a *better way to throw a punch*.

## 2. The map

A seeded hex world (the v1 ocean worldgen carries over conceptually — ~95% non-land, POIs
sprinkled, but water becomes Fog and "depth" becomes *thickness*).

### 2.1 Hex classes

| Hex | What it is | Travel | Notes |
|---|---|---|---|
| **Stable island** | Anchored reality: ports, settlements, masters' seats, trainers | free | the hubs; safe by default |
| **Fading island** | An island losing its anchor (master Crossed, weakened, or absent) | free, uneasy | a timer made visible; quests decide its fate |
| **Shallow fog** | Thin grey: navigable lanes between islands | normal cost | most travel; encounter tokens drift here |
| **Deep fog** | Thick unmaking; reality is negotiable | costs **Anchor** (§3.2) | shortcuts, secrets, the worst things |
| **Anomaly** | Fog-carved strangeness: half-eaten ruins, becalmed fleets, geometry that shouldn't | varies | set-piece content; dungeon entrances |
| **Threshold site** | Where Crossings happen | story-gated | the plot's holy/horror ground |

### 2.2 Stability and the Fog border

Every hex carries a **stability** value; the Fog border is its zero contour. Stability is not a
simulation the player babysits — it moves at **authored moments**: when a master Crosses, when
story gates open, when a faction wins something, and (post-Gate-3, see §7) in response to the
**Tithe**. Hexes that hit zero are *redrawn*: islands become anomalies, anomalies become deep
fog, lanes close. The map UI keeps a ghost of what was lost — the player should be able to
mourn specific places.

⚠️ Tuning stance: the border moves rarely and meaningfully (a campaign has perhaps a dozen
advances), never on a per-turn drip — dread, not attrition.

## 3. Travel

### 3.1 Moving

Hex-to-hex movement on a time-cost basis (a hex of shallow fog = one watch ⚠️). Time matters
because the world's actors keep their own schedules (caravans sail, rivals train, story clocks
tick) — not because of survival upkeep. **No food/water micromanagement**; the ship's needs are
narrative (port repairs after story damage), not a meter.

### 3.2 The Anchor (deep-fog diving)

The ship carries an **anchor-lantern** — a reality-keeping flame (charged at masters' islands,
upgradeable through story/loot). Deep-fog hexes cost Anchor charge per hex; running dry doesn't
kill you — it *strands* you: the ship drifts back to the nearest shallow lane with a scar
(a debuff, a lost cargo slot, a crew story ⚠️), and the deep route stays unexplored. Deep fog is
where shortcuts, anomalies, and the best wrecks are: Anchor is the push-your-luck currency of
the map layer.

### 3.3 Weather as fog texture

No weather sim; instead authored **fog states** on regions (becalmed / churning / whiteout
fronts) that modulate travel cost, token visibility radius, and encounter composition. They
move on the same authored-moment cadence as stability.

## 4. Encounters

### 4.1 Tokens, always visible

Encounters are **visible tokens** drifting on the map with simple authored behaviors (patrol,
lurk, pursue-if-close, flee). Sail into one — or let it catch you — and the **Combat overlay**
raises (`fsm.md`): the fight runs on the combat spec, the world freezes beneath.

### 4.2 Taxonomy

| Token | Who | Why fight them |
|---|---|---|
| **Corsairs** | Fogline deserters, freebooters | cargo, bounties, Form scraps (human movesets) |
| **Fog-beasts** | Things the deep fog grew | materials, anomaly keys; inhuman cue vocabularies (§4.3) |
| **Rival ascendants** | Named or generated duelists working their own ascent | knowledge (the codex!), standing, rare Form intel |
| **Hosts patrols / Concord wardens** | Faction muscle | political consequences either way |
| **Escorts & merchants** | Defended prizes | piracy is a real economy with real faction costs |

### 4.3 Enemy variety is cue variety

Enemy design language: a fog-beast isn't "a goblin with more HP" — it's a **different cue
vocabulary** (alien wind-ups, wrong-jointed tells) and a different RPS emphasis. Difficulty
tiers raise read complexity (deeper candidate sets, better AI priors), not raw stat walls.
The codex (matchup knowledge, `progression.md` §6) is the exploration-combat bridge: every new
token type is also a *learnable subject*.

### 4.4 Outcomes

Victory → **Loot** beat (§6) → back to sail. Escape → disengage with a map cost (drift,
token alert states ⚠️). Defeat → **soft loss**: the party wakes at the last anchorage with a
story scar and some cargo loss ⚠️ — never a reload wall (per fsm.md's decided soft-loss rule).

## 5. Points of interest

- **Ports / settlements** — shops (gear, affixed loot, intel), taverns (rumors = token and
  anomaly markers on your map), faction presences, the Witness's people.
- **Trainers** — attribute training and low-tier Form teaching for coin and time
  (`progression.md` §3–4).
- **Masters' seats** — the anchor islands. Each master: teaches a Form line to those with
  standing, holds **rank trials** (the no-XP spine of progression), radiates the stability that
  keeps their region real, and is a story actor who may one day Cross — taking their island's
  future with them. The exploration map and the plot are the same object here.
- **Dungeons** — fog-eaten ruins and anomalies entered from their hex: room-graph delves
  (authored layouts first; proc-gen deferred ⚠️) with visible encounters, chests, a boss, and a
  guaranteed high-tier drop. Some delves carry **authored** instability timers (the ruin is
  still dissolving) — pressure is a per-dungeon design choice, never a global rule.
- **Threshold sites** — story set-pieces; the Crossing scenes; late-game decision ground.

## 6. Loot

The Diablo/BL2 logic, but every affix speaks frame data:

```
drop = base item (weapon / armor / talisman / consumable / Form scrap / intel)
     × rarity tier (drop weights by source)
     × affix rolls (from pools gated by base + tier)
```

- **Weapons** carry the spacing identity (range/speed/damage triangle, audited R-4) and grant
  moves; **armor** trades defense-profile values against mobility; **talismans** carry
  property riders (e.g. "+1 armor hit on Iron Mountain stance moves," "Focus +2 on parry").
- **Affix pools are budget-audited** (spec §13): an affix is a priced delta, so no roll can
  mint a budget-violating move. Rarity raises the *number and interest* of deltas, not the
  ceiling above the law.
- **Form scraps & intel** are loot too: partial Form teaching (a move without its master's
  line) and codex knowledge (matchup tiers for coin) — knowledge is lootable because knowledge
  is power, literally (vision pillar 2).
- Generation is a seeded data system (C-DET applies out of combat too: same seed, same world,
  same drops).

## 7. The Tithe coupling

Every fight and every cultivation gain quietly feeds the hidden **Tithe** meter (bible §8).
Pre-Gate-3 it is invisible and *changes nothing the player can see*. Post-reveal, the UI shows
it — recontextualizing the whole run — and it gains one authored mechanical tooth: Tithe
milestones are among the triggers the Fog-border system listens to (§2.2). Fighting never stops
being fun; the map starts whispering about what the fun costs. ⚠️ The coupling strength is a
story-tuning knob, owned by the narrative team, OFF by default until Gate 3.

## 8. From hex to arena

Engaging an encounter authors the combat **ArenaDef** from the map context: hex class + fog
state + token type select a stage template — a ship deck (rails = walls, the sea = an
*overboard* hazard), a port quay, a dissolving ruin (breakable walls), a deep-fog clearing
(no walls at all, sidestep heaven ⚠️ watch the balance). Stage hazards are arena data
(spec §3.1); the map is never the combat space (two spatial models, cleanly separated — the
fsm.md rule carries over from v1).

## 9. Quarantine checklist (C-QUAR, enforced in review)

- No frame data, advantage numbers, or cue previews anywhere on the map or in menus.
- Encounter tokens telegraph *difficulty and faction*, never movesets. (The codex is consulted
  from the pause menu as *recorded* knowledge — a journal, not a lab.)
- Training with masters is a scene + a transaction, not a rhythm minigame.
- The loot screen names moves and qualities in RPG language; the deep numbers live one
  deliberate tap deeper for those who want them.
