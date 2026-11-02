use battery_notification_util::policy::*;

#[path = "cases/policy_cycles.rs"]
mod policy_cycles;

#[test]
fn custom_threshold_boundaries() {
    let thresholds = Thresholds {
        critical: 10.0,
        low: 25.0,
        rearm: 30.0,
    };
    for (percentage, expected) in [
        (10.0, Effect::Show(Level::Critical)),
        (10.001, Effect::Show(Level::Low)),
        (25.0, Effect::Show(Level::Low)),
        (25.001, Effect::None),
        (30.0, Effect::None),
        (30.001, Effect::Close),
    ] {
        let mut policy = Policy::new(thresholds).unwrap();
        assert_eq!(policy.evaluate(snapshot(percentage)).unwrap(), expected);
    }

    let mut policy = Policy::new(thresholds).unwrap();
    policy.issued(Level::Critical);
    assert_eq!(policy.evaluate(snapshot(30.0)).unwrap(), Effect::None);
    assert_eq!(policy.evaluate(snapshot(30.001)).unwrap(), Effect::Close);
    assert_eq!(
        policy.evaluate(snapshot(25.0)).unwrap(),
        Effect::Show(Level::Low)
    );
}

fn thresholds() -> Thresholds {
    Thresholds {
        critical: 5.0,
        low: 15.0,
        rearm: 20.0,
    }
}

fn snapshot(percentage: f64) -> Snapshot {
    Snapshot {
        on_battery: true,
        present: true,
        percentage,
        state: 2,
    }
}

#[test]
fn threshold_validation() {
    thresholds().validate().unwrap();
    Thresholds {
        critical: 0.0,
        low: 50.0,
        rearm: 100.0,
    }
    .validate()
    .unwrap();
    for values in [
        [-0.1, 15.0, 20.0],
        [5.0, 5.0, 20.0],
        [6.0, 5.0, 20.0],
        [5.0, 20.0, 20.0],
        [5.0, 21.0, 20.0],
        [5.0, 15.0, 100.1],
    ] {
        let invalid = Thresholds {
            critical: values[0],
            low: values[1],
            rearm: values[2],
        };
        assert!(invalid.validate().is_err());
        assert!(Policy::new(invalid).is_err());
    }
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for field in 0..3 {
            let mut values = [5.0, 15.0, 20.0];
            values[field] = invalid;
            assert!(
                Thresholds {
                    critical: values[0],
                    low: values[1],
                    rearm: values[2]
                }
                .validate()
                .is_err()
            );
        }
    }
}

#[test]
fn percentage_boundaries() {
    for (percentage, expected) in [
        (0.0, Effect::Show(Level::Critical)),
        (4.999, Effect::Show(Level::Critical)),
        (5.0, Effect::Show(Level::Critical)),
        (5.001, Effect::Show(Level::Low)),
        (14.999, Effect::Show(Level::Low)),
        (15.0, Effect::Show(Level::Low)),
        (15.001, Effect::None),
        (20.0, Effect::None),
        (20.001, Effect::Close),
        (100.0, Effect::Close),
    ] {
        let mut policy = Policy::new(thresholds()).unwrap();
        assert_eq!(policy.evaluate(snapshot(percentage)).unwrap(), expected);
    }
}

#[test]
fn invalid_active_percentage_errors_without_clearing_latch() {
    for percentage in [-0.001, 100.001, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut policy = Policy::new(thresholds()).unwrap();
        policy.issued(Level::Critical);
        assert!(policy.evaluate(snapshot(percentage)).is_err());
        assert_eq!(policy.evaluate(snapshot(5.0)).unwrap(), Effect::None);
        for (present, on_battery) in [(false, true), (true, false), (false, false)] {
            assert_eq!(
                policy
                    .evaluate(Snapshot {
                        present,
                        on_battery,
                        ..snapshot(percentage)
                    })
                    .unwrap(),
                Effect::Close
            );
        }
    }
}

#[test]
fn only_discharging_states_issue_warnings() {
    for state in [0, 1, 2, 3, 4, 5, 6, u32::MAX] {
        let mut policy = Policy::new(thresholds()).unwrap();
        assert_eq!(
            policy
                .evaluate(Snapshot {
                    state,
                    ..snapshot(5.0)
                })
                .unwrap(),
            if matches!(state, 2 | 6) {
                Effect::Show(Level::Critical)
            } else {
                Effect::None
            }
        );
    }
}
