use vibetracer::analysis::watchdog::Watchdog;
use vibetracer::config::WatchdogConstant;

fn earth_radius_rule() -> WatchdogConstant {
    WatchdogConstant {
        file: "src/geo.rs".to_string(),
        pattern: r"EARTH_RADIUS\s*=\s*([\d.]+)".to_string(),
        expected: "6371.0".to_string(),
        severity: "error".to_string(),
    }
}

#[test]
fn test_watchdog_detects_constant_change() {
    let watchdog = Watchdog::new(vec![earth_radius_rule()]);

    let old_content = "EARTH_RADIUS = 6371.0";
    let new_content = "EARTH_RADIUS = 6400.0";

    let alerts = watchdog.check("src/geo.rs", old_content, new_content);

    assert_eq!(alerts.len(), 1, "expected one alert");
    let alert = &alerts[0];
    assert_eq!(alert.expected, "6371.0");
    assert_eq!(alert.actual, "6400.0");
    assert_eq!(alert.severity, "error");
    assert_eq!(alert.file, "src/geo.rs");
}

#[test]
fn test_watchdog_no_alert_when_unchanged() {
    let watchdog = Watchdog::new(vec![earth_radius_rule()]);

    let content = "EARTH_RADIUS = 6371.0";

    let alerts = watchdog.check("src/geo.rs", content, content);

    assert!(alerts.is_empty(), "expected no alerts when value is unchanged");
}
