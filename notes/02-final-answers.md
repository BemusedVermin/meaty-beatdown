# Final clarifying answers — 2026-06-09 (round 4, free-form)

1. **Prediction UI confirmed**: deterministic preview of your committed move vs the
   opponent's last-observable state. Honest but invalidatable → feints/tricks are a thing.
2. **Enemy meters**: only HP visible at first; per-meter visibility must be TUNABLE.
3. **AI fog**: cue-based profiles, bosses get authored smart-reads, never peeks at commits.
4. **Heights**: Tekken logic (highs whiff crouch, mids beat crouch, lows seeable). Drop OVERHEAD.
5. **Throw breaks**: 2-way directional read. Knowledge can reveal break hints.
6. **Juggle grammar**: Tekken extender states, BUT EMPHASIZE **NO INFINITE COMBOS**.
   (→ make the anti-infinite charter a first-class spec section; extenders once-per-combo.)
7. **Combo relief**: ally interruption (largely EMERGENT — third party hits the comboer;
   plus an authored costed Rescue move class) AND a solo burst when alone.
8. **Spatial: TARGET-LANE in a fully 3D environment.** Sidesteps put people into different
   lanes; every actor targets exactly one other; **the target creates the lane**. All v1
   lane math survives as the per-pair special case (pos = distance along line-of-sight,
   offset = perpendicular deviation). User: "my main idea", revisit later if needed.
9. **KO**: companions KO-able + revivable; loss only on full party wipe.
10. **Meters**: the buildable super gauge is called **FOCUS** (absorbs old Focus + Ki gauge).
    Final: HP, Breath (exertion), Guard (poise), Focus (earned/built super gauge), AP (tempo)
    + Heat (state) + Rage (state).
11. **Authored qualities + orthogonal move axes**: both confirmed into spec v2.
    No engine combat constants — CH bonus, parry freeze, etc. all authored data.
12. **Setting change: FLOOD → FOG.** The fog **eats reality** (existence-erasure).
    Masters hold back the fog (anchor islands of stability). **Main villain is entropy.**
    Exploration spine (hexcrawl, visible encounters, masters' islands, dungeons, loot) stays.
13. **No XP levels.** Rank/trials/training/Forms/loot/knowledge are the progression axes.
14. **Tithe meter stays** in the design (hidden → revealed at Gate 3).
15. **Milestone: vertical slice** (island chain, party of 3, dungeon + boss, supers/Heat/
    walls working, loot loop closed).
16. **Golden vectors v2**: regenerate from the Rust engine once it stabilizes.
17. **Docs plan approved**: rewrite spec + mechanics in place, update fsm.md, new docs for
    exploration/progression/MDA/implementation, plot bible light-touch fog pass.
    **Keep working title TICK.**

## Naming worked out for the fog re-theme (all [?] placeholders preserved)
- The Tide → **the Fog [?]** — existence-eating mist; advances fastest where most Cross.
- The Undertow → **the Hollow [?]** — the entropic force inside the deep Fog.
- The Drowned → **the Faded [?]** — refugees of erased homelands (some partially erased).
- Tidemark Hosts → **Fogline Hosts [?]**; the Stillwater → **the Unmoved [?]**.
- The Ferrymen → KEEP (Charon ferries through mist; still lands).
- The deluge → **the Whiteout [?]** (stop the harvest and the Fog comes all at once).
- NEW canonical truth: masters anchor reality; a master's Crossing starts their island fading.
