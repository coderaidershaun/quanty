//! The checks that a window leaves room for what it shows: every part lies inside its room and the
//! window, no two parts cover each other, and a control is tall enough for a finger.

use eframe::egui::{Pos2, Rect, Vec2};

/// Rects are kept to half a point, so a part may be that far out of place and still fit.
pub(super) const SLACK: f32 = 0.5;

/// The rectangle of a window of that size.
pub(crate) fn window_rect(window: [f32; 2]) -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::from(window))
}

/// Whether two parts cover each other by more than the rounding of a rect.
pub(super) fn overlaps(a: Rect, b: Rect) -> bool {
    let both = a.intersect(b);
    both.width() > SLACK && both.height() > SLACK
}

/// Every part lies inside `room` and inside `window`, and no two parts overlap.
pub(crate) fn assert_inside_and_apart(parts: &[(String, Rect)], room: Rect, window: Rect) {
    // A search that finds nothing would leave nothing to check, and the test would pass without
    // looking at the window.
    assert!(!parts.is_empty(), "no part was found to fit in {room:?}");
    let (room, window) = (room.expand(SLACK), window.expand(SLACK));
    for (name, rect) in parts {
        assert!(
            room.contains_rect(*rect) && window.contains_rect(*rect),
            "`{name}` {rect:?} is outside the room {room:?} or the window {window:?}"
        );
    }
    for (place, (name, rect)) in parts.iter().enumerate() {
        for (other_name, other) in &parts[place + 1..] {
            assert!(
                !overlaps(*rect, *other),
                "`{name}` {rect:?} overlaps `{other_name}` {other:?}"
            );
        }
    }
}

/// A control that a finger presses is at least `least` points tall.
pub(super) fn assert_finger_sized(name: &str, rect: Rect, least: f32) {
    assert!(
        rect.height() >= least,
        "`{name}` is {} tall, and a finger needs {least}",
        rect.height()
    );
}
