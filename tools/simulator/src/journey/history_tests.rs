use super::*;

#[test]
fn old_join_cursor_gap_is_reconciled_only_by_a_continuous_actual_cursor_poll() {
    let acknowledged = game::WorldSnapshot {
        revision: 500,
        tick: 400,
        next_sequence: 70,
        event_history_gap: true,
        event_history_floor_revision: 250,
        ..Default::default()
    };
    let recovered = game::WorldSnapshot {
        revision: 501,
        tick: 401,
        next_sequence: 70,
        event_history_gap: false,
        event_history_floor_revision: 251,
        ..Default::default()
    };
    assert!(Runner::validate_history_reconciliation(499, &acknowledged, &recovered).is_ok());
    assert!(Runner::validate_history_reconciliation(200, &acknowledged, &recovered).is_err());
    let mut invalid = recovered.clone();
    invalid.event_history_gap = true;
    assert!(Runner::validate_history_reconciliation(499, &acknowledged, &invalid).is_err());
    invalid = recovered.clone();
    invalid.next_sequence += 1;
    assert!(Runner::validate_history_reconciliation(499, &acknowledged, &invalid).is_err());
    invalid = recovered;
    invalid.revision = 498;
    assert!(Runner::validate_history_reconciliation(499, &acknowledged, &invalid).is_err());
}

#[test]
fn chronological_suffix_requires_the_exact_last_observed_event_not_just_an_old_id() {
    let last = game::Event {
        event_id: "event.observed".into(),
        actor_id: "actor.synthetic".into(),
        kind: "xp_gained".into(),
        xp_tenths: 100,
        ..Default::default()
    };
    let next = game::Event {
        event_id: "event.new".into(),
        actor_id: last.actor_id.clone(),
        kind: "interface_opened".into(),
        ..Default::default()
    };
    assert!(Runner::history_suffix_covered(
        Some(&last),
        &[last.clone(), next.clone()]
    ));
    assert!(!Runner::history_suffix_covered(Some(&last), &[next]));
    let mut altered = last.clone();
    altered.xp_tenths = 0;
    assert!(!Runner::history_suffix_covered(Some(&last), &[altered]));
    assert!(!Runner::history_suffix_covered(
        Some(&last),
        &[last.clone(), last.clone()]
    ));
    assert!(!Runner::history_suffix_covered(None, &[last]));
}
