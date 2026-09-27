use inklift_shot::Rect;

#[test]
fn a_rect_parses_from_the_region_flag_format() {
    let r: Rect = "100,50,400,200".parse().unwrap();
    assert_eq!((r.x, r.y, r.width, r.height), (100, 50, 400, 200));
}

#[test]
fn parsing_tolerates_spaces_and_rejects_nonsense() {
    assert!(" 10, 20 , 30,40 ".parse::<Rect>().is_ok());
    for bad in ["", "1,2,3", "1,2,3,4,5", "a,b,c,d", "1,2,-3,4", "1,2,0,4"] {
        assert!(bad.parse::<Rect>().is_err(), "{bad:?} should be rejected");
    }
}

/// Negative origins are legal: a monitor left of the primary has negative x.
#[test]
fn a_negative_origin_is_accepted_but_a_negative_size_is_not() {
    assert!("-1920,0,800,600".parse::<Rect>().is_ok());
    assert!("0,0,800,-600".parse::<Rect>().is_err());
}

/// Dragging bottom-right to top-left must give the same rect as the reverse.
#[test]
fn a_backwards_drag_normalises_to_the_same_rect() {
    let forward = Rect::from_corners(10, 20, 110, 220);
    let backward = Rect::from_corners(110, 220, 10, 20);
    let mixed = Rect::from_corners(110, 20, 10, 220);

    assert_eq!(forward, backward);
    assert_eq!(forward, mixed);
    assert_eq!((forward.x, forward.y, forward.width, forward.height), (10, 20, 100, 200));
}

#[test]
fn a_zero_drag_is_an_empty_rect() {
    let r = Rect::from_corners(50, 50, 50, 50);
    assert!(r.is_empty());
    assert_eq!(r.area(), 0);
}

#[test]
fn clamping_keeps_a_rect_inside_its_bounds() {
    let bounds = Rect::new(0, 0, 1920, 1080);

    let inside = Rect::new(100, 100, 200, 200).clamped_to(&bounds).unwrap();
    assert_eq!(inside, Rect::new(100, 100, 200, 200));

    // Overhanging the right and bottom edges gets trimmed, not moved.
    let over = Rect::new(1800, 1000, 400, 400).clamped_to(&bounds).unwrap();
    assert_eq!(over, Rect::new(1800, 1000, 120, 80));

    // Starting off the left edge trims the left side.
    let left = Rect::new(-50, 10, 200, 100).clamped_to(&bounds).unwrap();
    assert_eq!(left, Rect::new(0, 10, 150, 100));
}

#[test]
fn a_rect_entirely_outside_its_bounds_clamps_to_nothing() {
    let bounds = Rect::new(0, 0, 1920, 1080);
    assert!(Rect::new(5000, 5000, 100, 100).clamped_to(&bounds).is_none());
    assert!(Rect::new(-500, 0, 100, 100).clamped_to(&bounds).is_none());
}

/// Monitors can sit at negative offsets, so a rect must be expressible relative
/// to the monitor that contains it.
#[test]
fn a_rect_translates_into_monitor_local_coordinates() {
    let monitor = Rect::new(-1920, 0, 1920, 1080);
    let global = Rect::new(-1820, 100, 300, 200);

    let local = global.relative_to(&monitor);

    assert_eq!(local, Rect::new(100, 100, 300, 200));
}

#[test]
fn a_rect_reports_whether_it_meets_a_minimum_size() {
    assert!(!Rect::new(0, 0, 4, 4).is_at_least(8, 8), "a misclick");
    assert!(Rect::new(0, 0, 8, 8).is_at_least(8, 8));
    assert!(!Rect::new(0, 0, 200, 4).is_at_least(8, 8), "too thin counts too");
}

#[test]
fn a_rect_renders_back_to_the_flag_format() {
    assert_eq!(Rect::new(10, 20, 30, 40).to_string(), "10,20,30,40");
    assert_eq!("10,20,30,40".parse::<Rect>().unwrap().to_string(), "10,20,30,40");
}
