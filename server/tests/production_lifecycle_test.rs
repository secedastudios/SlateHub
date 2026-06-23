//! Unit tests for the production lifecycle phase mapping that drives the
//! management-overview stepper. Pure logic — no test DB required.

use slatehub::models::production::{LifecyclePhase, LifecycleView};

#[test]
fn maps_canonical_status_strings_to_phases() {
    let cases = [
        ("Development", LifecyclePhase::Development),
        ("Pre-Production", LifecyclePhase::PreProduction),
        ("Production", LifecyclePhase::Production),
        ("Post-Production", LifecyclePhase::PostProduction),
        (
            "Marketing/Distribution",
            LifecyclePhase::MarketingDistribution,
        ),
        ("Released", LifecyclePhase::Released),
    ];
    for (status, expected) in cases {
        assert_eq!(
            LifecyclePhase::from_status(status),
            expected,
            "status {status:?} should map to {expected:?}"
        );
    }
}

#[test]
fn mapping_is_tolerant_of_casing_and_separators() {
    for variant in [
        "pre-production",
        "pre_production",
        "PREPRODUCTION",
        "Pre Production",
    ] {
        assert_eq!(
            LifecyclePhase::from_status(variant),
            LifecyclePhase::PreProduction,
            "{variant:?} should normalize to Pre-Production"
        );
    }
}

#[test]
fn maps_legacy_production_status_vocabulary() {
    // The live `production_status` reference table predates the six canonical
    // phases — its extra values must fold into the right phase.
    assert_eq!(
        LifecyclePhase::from_status("Completed"),
        LifecyclePhase::MarketingDistribution
    );
    assert_eq!(
        LifecyclePhase::from_status("Festival"),
        LifecyclePhase::MarketingDistribution
    );
    assert_eq!(
        LifecyclePhase::from_status("Pre-Sales"),
        LifecyclePhase::MarketingDistribution
    );
    assert_eq!(
        LifecyclePhase::from_status("Canceled"),
        LifecyclePhase::Canceled
    );
}

#[test]
fn unknown_status_falls_back_to_development() {
    assert_eq!(
        LifecyclePhase::from_status("something-weird"),
        LifecyclePhase::Development
    );
    assert_eq!(LifecyclePhase::from_status(""), LifecyclePhase::Development);
}

#[test]
fn canceled_has_no_linear_order() {
    assert_eq!(LifecyclePhase::Canceled.order(), None);
    assert_eq!(LifecyclePhase::Development.order(), Some(0));
    assert_eq!(LifecyclePhase::Released.order(), Some(5));
}

#[test]
fn view_marks_reached_steps_up_to_and_including_current() {
    let view = LifecycleView::from_status("Production"); // index 2
    assert_eq!(view.steps.len(), 6);

    // Development, Pre-Production, Production reached; rest not.
    let reached: Vec<bool> = view.steps.iter().map(|s| s.reached).collect();
    assert_eq!(reached, vec![true, true, true, false, false, false]);

    // Exactly one current, and it's Production.
    let current: Vec<bool> = view.steps.iter().map(|s| s.current).collect();
    assert_eq!(current, vec![false, false, true, false, false, false]);
    assert_eq!(view.current_label, "Production");
    assert!(!view.canceled);
}

#[test]
fn released_marks_every_step_reached() {
    let view = LifecycleView::from_status("Released");
    assert!(view.steps.iter().all(|s| s.reached));
    assert!(view.steps.last().unwrap().current);
    assert_eq!(view.current_key, "released");
}

#[test]
fn canceled_view_reaches_nothing_and_flags_canceled() {
    let view = LifecycleView::from_status("Canceled");
    assert!(view.canceled);
    assert!(
        view.steps.iter().all(|s| !s.reached && !s.current),
        "canceled is off the linear flow — no step is reached or current"
    );
    assert_eq!(view.current_label, "Canceled");
}

#[test]
fn step_numbers_are_one_based_and_sequential() {
    let view = LifecycleView::from_status("Development");
    let numbers: Vec<usize> = view.steps.iter().map(|s| s.number).collect();
    assert_eq!(numbers, vec![1, 2, 3, 4, 5, 6]);
}
