use inklift_shot::{Monitor, Rect, monitor_at, monitor_for, virtual_bounds};

fn dual() -> Vec<Monitor> {
    vec![
        Monitor { name: "left".into(), bounds: Rect::new(-1920, 0, 1920, 1080), primary: false },
        Monitor { name: "main".into(), bounds: Rect::new(0, 0, 2560, 1080), primary: true },
    ]
}

#[test]
fn virtual_bounds_span_every_monitor() {
    let b = virtual_bounds(&dual()).unwrap();
    assert_eq!(b, Rect::new(-1920, 0, 4480, 1080));
}

#[test]
fn virtual_bounds_cope_with_monitors_at_different_heights() {
    let screens = vec![
        Monitor { name: "a".into(), bounds: Rect::new(0, 0, 1920, 1080), primary: true },
        Monitor { name: "b".into(), bounds: Rect::new(1920, -200, 1080, 1920), primary: false },
    ];
    assert_eq!(virtual_bounds(&screens).unwrap(), Rect::new(0, -200, 3000, 1920));
}

#[test]
fn virtual_bounds_of_nothing_is_nothing() {
    assert!(virtual_bounds(&[]).is_none());
}

#[test]
fn a_point_resolves_to_the_monitor_under_it() {
    let screens = dual();
    assert_eq!(monitor_at(&screens, 100, 100).unwrap().name, "main");
    assert_eq!(monitor_at(&screens, -100, 100).unwrap().name, "left");
    assert!(monitor_at(&screens, 99_999, 0).is_none());
}

/// A selection dragged across a seam belongs to whichever screen holds most of
/// it, so the capture comes from one frame rather than being stitched.
#[test]
fn a_straddling_region_picks_the_monitor_holding_most_of_it() {
    let screens = dual();

    let mostly_left = Rect::new(-300, 100, 400, 100); // 300px left, 100px main
    assert_eq!(monitor_for(&screens, &mostly_left).unwrap().name, "left");

    let mostly_main = Rect::new(-100, 100, 400, 100); // 100px left, 300px main
    assert_eq!(monitor_for(&screens, &mostly_main).unwrap().name, "main");
}

#[test]
fn a_region_off_every_monitor_resolves_to_none() {
    assert!(monitor_for(&dual(), &Rect::new(90_000, 90_000, 10, 10)).is_none());
}

#[test]
fn a_single_monitor_setup_is_not_a_special_case() {
    let one = vec![Monitor { name: "only".into(), bounds: Rect::new(0, 0, 2560, 1080), primary: true }];
    assert_eq!(virtual_bounds(&one).unwrap(), Rect::new(0, 0, 2560, 1080));
    assert_eq!(monitor_for(&one, &Rect::new(10, 10, 100, 100)).unwrap().name, "only");
}
