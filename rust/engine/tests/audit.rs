//! The content audit as a test gate (spec §13.4; standing rule 1: the audit is a merge
//! gate from the phase that introduces it). Shipped content joins in Phase 6; until
//! then the gate runs over the test kit.

mod common;

use engine::content::audit::audit;
use engine::data::movedef::{CancelGate, CancelWindow, GainGate, GainResource, ResourceGain};

#[test]
fn the_test_kit_passes_the_audit() {
    let report = audit(&common::kit(), &common::ruleset());
    report.assert_clean();
    // The governor-7 analytic bound: max stun 40 / step 2 + three latches + slack.
    assert_eq!(report.combo_bound, 25);
}

#[test]
fn r5_rejects_a_non_negative_cancel_cycle() {
    let mut moves = common::kit();
    // Sabotage: a free self-chain on the palm whose conditional gains cover its cost.
    let palm = moves
        .iter_mut()
        .find(|m| m.id == common::JUGGLE_PALM)
        .unwrap();
    palm.cost.ap = 0;
    palm.gains.push(ResourceGain {
        resource: GainResource::Ap,
        amount: 5,
        gate: GainGate::OnHit,
    });
    palm.cancels = vec![CancelWindow {
        from: 9,
        to: 14,
        gate: CancelGate::OnHit,
        into: common::JUGGLE_PALM,
        ap_cost: 0,
        focus_cost: 0,
    }];
    let report = audit(&moves, &common::ruleset());
    assert!(
        report.errors.iter().any(|e| e.contains("R-5")),
        "a self-sustaining loop must be caught: {:?}",
        report.errors
    );
}

#[test]
fn r6_requires_decay_when_launchers_exist() {
    let mut rs = common::ruleset();
    rs.hitstun_decay_step = 0;
    let report = audit(&common::kit(), &rs);
    assert!(
        report.errors.iter().any(|e| e.contains("R-6")),
        "{:?}",
        report.errors
    );
}

#[test]
fn sanity_rejects_malformed_authoring() {
    let mut moves = common::kit();
    // A cancel window reaching into startup without the feint flag.
    let jab = moves.iter_mut().find(|m| m.id == common::JAB).unwrap();
    jab.cancels.push(CancelWindow {
        from: 0,
        to: 4,
        gate: CancelGate::Always,
        into: common::MID_POKE,
        ap_cost: 1,
        focus_cost: 0,
    });
    let report = audit(&moves, &common::ruleset());
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.contains("startup_cancelable"))
    );
}
