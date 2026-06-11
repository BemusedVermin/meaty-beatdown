//! The content audit, first cut (spec §13.4; implementation-plan Phase 2): balance as a
//! checkable property. This phase covers data sanity (I-1 adjacent), rule R-5 / governor
//! 6 (no non-negative AP+Focus cycle in the cancel graph), and the governor-7 juggle
//! termination bound. Budget residuals, R-1..R-4/R-7, and the RPS matrix join with the
//! budget identity in Phase 5–6.
//!
//! Graph machinery: `petgraph` builds the cancel graph and finds SCCs (library policy).
//! The non-negative-cycle scan itself is an integer Bellman–Ford: petgraph's shortest
//! path APIs are float-based, and floats are banned crate-wide (C-DET) — the one case
//! where the policy's "declining a library is legitimate when it would cost correctness"
//! clause applies to a single function.

use crate::data::movedef::{GainGate, GainResource, Move, MoveCategory, Tracking};
use crate::data::{MoveId, Reaction, Ruleset};
use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

/// The audit's verdict. `errors` non-empty = the content does not ship.
#[derive(Clone, Debug, Default)]
pub struct AuditReport {
    pub errors: Vec<String>,
    /// The proven worst-case combo length under the Ruleset's decay (governor 7 bound);
    /// property tests assert fuzzed fights never exceed it.
    pub combo_bound: u32,
    /// Structural Phase 5 budget-axis coverage. Numeric residuals remain playtest-owned.
    pub budget_axes: BudgetAxes,
}

impl AuditReport {
    /// Panic with every finding (test/CI gate ergonomics).
    pub fn assert_clean(&self) {
        assert!(
            self.errors.is_empty(),
            "audit failed:\n{}",
            self.errors.join("\n")
        );
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct BudgetAxes {
    pub wide_arc: u32,
    pub tracking: u32,
    pub meter: u32,
    pub cue_lie: u32,
    pub heat: u32,
    pub projectile: u32,
    pub super_move: u32,
}

/// Run the Phase 2 audit over a movelist + ruleset.
#[must_use]
pub fn audit(moves: &[Move], ruleset: &Ruleset) -> AuditReport {
    let mut report = AuditReport::default();
    sanity(moves, &mut report);
    budget_axes(moves, &mut report);
    cancel_cycles(moves, &mut report);
    report.combo_bound = juggle_termination(moves, ruleset, &mut report);
    report
}

/// Data sanity: the schema can express what the engine cannot honor; the audit refuses it.
fn sanity(moves: &[Move], report: &mut AuditReport) {
    let ids: HashMap<MoveId, &Move> = moves.iter().map(|m| (m.id, m)).collect();
    if ids.len() != moves.len() {
        report.errors.push("duplicate move ids".into());
    }
    for m in moves {
        let err = |report: &mut AuditReport, what: &str| {
            report
                .errors
                .push(format!("{} ({:?}): {what}", m.name, m.id));
        };
        match m.category {
            MoveCategory::Strike | MoveCategory::Projectile => {
                if m.hits.is_empty() && m.properties.is_empty() {
                    err(report, "strike with no hits and no properties");
                }
                for h in &m.hits {
                    if h.at >= m.timing.active {
                        err(report, "hit offset outside the active window");
                    }
                }
            }
            MoveCategory::Throw => {
                if m.hits.is_empty() {
                    err(report, "throw with no hits");
                }
            }
            MoveCategory::Stance => {
                if m.stance_spec.is_none() {
                    err(report, "stance move without a stance spec");
                }
                if m.timing.startup == 0 {
                    err(
                        report,
                        "stance startup must be >= 1 (exact-tick hold entry)",
                    );
                }
                if m.timing.active != 0 {
                    err(report, "stances author active = 0 (the hold is open-ended)");
                }
            }
            MoveCategory::Motion | MoveCategory::Utility => {}
        }
        if m.flags.heat_only && m.flags.heat_burst {
            err(report, "move cannot be both heat_only and heat_burst");
        }
        if m.flags.rage_art && m.flags.heat_only {
            err(
                report,
                "rage_art and heat_only are separate escalation gates",
            );
        }
        if let Some(p) = m.flags.projectile {
            if p.spawn_at >= m.timing.active {
                err(report, "projectile spawn offset outside active window");
            }
            if p.lifetime == 0 || p.speed <= crate::core::fx::Fx::ZERO {
                err(report, "projectile must have positive speed and lifetime");
            }
        }
        for w in &m.properties {
            if w.from > w.to || w.to >= m.timing.total() {
                err(report, "property window outside the move");
            }
        }
        for c in &m.cancels {
            if c.from > c.to || c.to >= m.timing.total() {
                err(report, "cancel window outside the move");
            }
            if c.from < m.timing.startup && !m.startup_cancelable {
                err(
                    report,
                    "cancel window covers startup without startup_cancelable",
                );
            }
            if !ids.contains_key(&c.into) {
                err(report, "cancel into an unknown move");
            }
        }
        // R-5 precondition: gains are conditional, never unconditional AP (spec §9.4).
        for g in &m.gains {
            if g.gate == GainGate::Always && g.resource == GainResource::Ap {
                err(report, "unconditional AP gain (R-5)");
            }
        }
    }
}

fn budget_axes(moves: &[Move], report: &mut AuditReport) {
    let mut cue_counts: HashMap<_, u32> = HashMap::new();
    for m in moves {
        *cue_counts.entry((m.form, m.cue)).or_default() += 1;
    }
    for m in moves {
        if m.region.arc_halfwidth > crate::core::fx::Fx::ONE {
            report.budget_axes.wide_arc += 1;
        }
        if !matches!(m.tracking, Tracking::Linear) {
            report.budget_axes.tracking += 1;
        }
        if !m.gains.is_empty() || m.cost.focus > 0 {
            report.budget_axes.meter += 1;
        }
        if cue_counts.get(&(m.form, m.cue)).copied().unwrap_or(0) > 1 {
            report.budget_axes.cue_lie += 1;
        }
        if m.flags.heat_burst || m.flags.heat_engager || m.flags.heat_only {
            report.budget_axes.heat += 1;
        }
        if m.flags.projectile.is_some() || m.category == MoveCategory::Projectile {
            report.budget_axes.projectile += 1;
        }
        if m.flags.super_move || m.flags.ex || m.flags.rage_art {
            report.budget_axes.super_move += 1;
        }
    }
}

/// Governor 6 / rule R-5: the cancel graph may contain no cycle whose net AP+Focus
/// gain is >= 0 — every loop must drain the tempo budget, even assuming every
/// conditional gain pays out (worst case for the defender).
fn cancel_cycles(moves: &[Move], report: &mut AuditReport) {
    let mut graph: DiGraph<MoveId, i64> = DiGraph::new();
    let mut nodes: HashMap<MoveId, NodeIndex> = HashMap::new();
    for m in moves {
        nodes.insert(m.id, graph.add_node(m.id));
    }
    let by_id: HashMap<MoveId, &Move> = moves.iter().map(|m| (m.id, m)).collect();
    for m in moves {
        for c in &m.cancels {
            let Some(target) = by_id.get(&c.into) else {
                continue;
            };
            // Net gain of traversing this edge: the target's maximum possible AP+Focus
            // gains minus everything paid to take it.
            let gains: i64 = target
                .gains
                .iter()
                .filter(|g| matches!(g.resource, GainResource::Ap | GainResource::Focus))
                .map(|g| i64::from(g.amount))
                .sum();
            let costs = i64::from(c.ap_cost)
                + i64::from(c.focus_cost)
                + i64::from(target.cost.ap)
                + i64::from(target.cost.focus);
            let net = gains - costs;
            graph.add_edge(nodes[&m.id], nodes[&c.into], net);
        }
    }

    // A non-negative cycle exists iff, after scaling (w' = -w * (E+1) - 1), a negative
    // cycle exists. Scan per SCC with integer Bellman–Ford.
    let edge_count = i64::try_from(graph.edge_count()).expect("few edges") + 1;
    for scc in tarjan_scc(&graph) {
        if scc.len() == 1 {
            let n = scc[0];
            if !graph.edges_connecting(n, n).any(|_| true) {
                continue; // no self-loop: a lone node can't cycle
            }
        }
        if bellman_ford_negative_cycle(&graph, &scc, edge_count) {
            let names: Vec<String> = scc.iter().map(|n| format!("{:?}", graph[*n])).collect();
            report.errors.push(format!(
                "R-5: the cancel graph has a cycle with non-negative net AP+Focus through {}",
                names.join(", ")
            ));
        }
    }
}

/// Integer Bellman–Ford negative-cycle detection restricted to one SCC, on weights
/// scaled so that "negative" captures original cycles with sum >= 0.
fn bellman_ford_negative_cycle(
    graph: &DiGraph<MoveId, i64>,
    scc: &[NodeIndex],
    scale: i64,
) -> bool {
    let in_scc: std::collections::HashSet<NodeIndex> = scc.iter().copied().collect();
    let index_of: HashMap<NodeIndex, usize> =
        scc.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let mut dist = vec![0i64; scc.len()];
    let edges: Vec<(usize, usize, i64)> = graph
        .edge_indices()
        .filter_map(|e| {
            let (a, b) = graph.edge_endpoints(e)?;
            if in_scc.contains(&a) && in_scc.contains(&b) {
                let w = -graph[e] * scale - 1;
                Some((index_of[&a], index_of[&b], w))
            } else {
                None
            }
        })
        .collect();
    for _ in 0..scc.len() {
        let mut changed = false;
        for &(a, b, w) in &edges {
            if dist[a] + w < dist[b] {
                dist[b] = dist[a] + w;
                changed = true;
            }
        }
        if !changed {
            return false;
        }
    }
    // Still relaxing after |V| rounds: a negative (i.e. original non-negative) cycle.
    let mut extra = false;
    for &(a, b, w) in &edges {
        if dist[a] + w < dist[b] {
            extra = true;
        }
    }
    extra
}

/// Governor 7 bound: under the Ruleset's hitstun decay, the worst-case combo length is
/// finite and computable — `max_stun / decay_step` hits before any stun undercuts every
/// pickup, plus the latched extenders. The property suite asserts fuzzed fights stay
/// under this.
fn juggle_termination(moves: &[Move], ruleset: &Ruleset, report: &mut AuditReport) -> u32 {
    let launches = moves.iter().any(|m| {
        m.hits.iter().any(|h| {
            matches!(h.reaction, Reaction::Launch { .. })
                || matches!(h.ch_reaction, Some(Reaction::Launch { .. }))
        })
    });
    let max_stun = moves
        .iter()
        .flat_map(|m| m.hits.iter())
        .flat_map(|h| [Some(h.reaction), h.ch_reaction].into_iter().flatten())
        .map(|r| match r {
            Reaction::Hitstun { ticks } | Reaction::Crumple { ticks } => ticks,
            Reaction::Launch { stun, .. } | Reaction::Screw { stun, .. } => stun,
            Reaction::Bound { stun } => stun,
            Reaction::Knockdown { .. } | Reaction::Push { .. } => 0,
        })
        .max()
        .unwrap_or(0);
    if launches && ruleset.hitstun_decay_step == 0 {
        report.errors.push(
            "R-6: launchers exist but hitstun_decay_step is 0 — juggles cannot be proven to terminate"
                .into(),
        );
        return u32::MAX;
    }
    if max_stun == 0 {
        return 1;
    }
    let latches = ruleset.extender_latches;
    max_stun / ruleset.hitstun_decay_step.max(1)
        + latches.screw
        + latches.bound
        + latches.wall_splat
        + 2
}
