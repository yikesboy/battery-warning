use super::*;

#[test]
fn evaluation_retries_until_successful_spawn() {
    let mut policy = Policy::new(thresholds()).unwrap();
    for level in [Level::Low, Level::Critical] {
        let input = snapshot(if level == Level::Low { 15.0 } else { 5.0 });
        for _ in 0..3 {
            assert_eq!(policy.evaluate(input).unwrap(), Effect::Show(level));
        }
        policy.issued(level);
        assert_eq!(policy.evaluate(input).unwrap(), Effect::None);
    }
}

#[test]
fn critical_never_downgrades_and_acknowledgement_has_no_event() {
    let mut policy = Policy::new(thresholds()).unwrap();
    policy.issued(Level::Critical);
    // Acknowledging or closing a dialog makes no policy call and keeps the latch.
    for percentage in [5.0, 10.0, 15.0, 19.0, 20.0, 0.0] {
        assert_eq!(policy.evaluate(snapshot(percentage)).unwrap(), Effect::None);
    }
    policy.issued(Level::Low);
    assert_eq!(policy.evaluate(snapshot(5.0)).unwrap(), Effect::None);
}

#[test]
fn low_latch_survives_recovery_up_to_rearm() {
    let mut policy = Policy::new(thresholds()).unwrap();
    policy.issued(Level::Low);
    for percentage in [15.001, 20.0, 15.0] {
        assert_eq!(policy.evaluate(snapshot(percentage)).unwrap(), Effect::None);
    }
    assert_eq!(
        policy.evaluate(snapshot(5.0)).unwrap(),
        Effect::Show(Level::Critical)
    );
}

#[test]
fn each_reset_condition_rearms_both_levels() {
    for level in [Level::Low, Level::Critical] {
        for reset in [
            Snapshot {
                on_battery: false,
                ..snapshot(f64::NAN)
            },
            Snapshot {
                present: false,
                ..snapshot(f64::NAN)
            },
            snapshot(20.001),
        ] {
            let mut policy = Policy::new(thresholds()).unwrap();
            policy.issued(level);
            assert_eq!(policy.evaluate(reset).unwrap(), Effect::Close);
            assert_eq!(
                policy.evaluate(snapshot(15.0)).unwrap(),
                Effect::Show(Level::Low)
            );
            assert_eq!(
                policy.evaluate(snapshot(5.0)).unwrap(),
                Effect::Show(Level::Critical)
            );
        }
    }
}
