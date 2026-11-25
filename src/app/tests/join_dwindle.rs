//! `join` on layouts that windows opened through the real insertion path
//! build, in default Dwindle and in Columns, on a 1280×800 display.

use super::dwindle::{landscape_outputs, open_dwindle_windows, restart_runtime};
use super::join::{focus_on, rect};
use super::*;

fn join(runtime: &mut WorkspaceRuntime<FakeBackend>, id: u32, direction: Direction) {
    focus_on(runtime, id);
    runtime
        .join_focused(direction, landscape_outputs(), &AtomicBool::new(false))
        .unwrap();
}

fn columns_config() -> Config {
    Config::parse("version = 1\n[settings]\nlayout = \"columns\"\n").unwrap()
}

/// One observation cycle on the landscape display with exactly `open` on
/// screen and `focused` focused: a window new to `open` is a new tile, one
/// missing from it has closed.
fn observe_landscape(runtime: &mut WorkspaceRuntime<FakeBackend>, open: &[u32], focused: u32) {
    let windows = open
        .iter()
        .map(|&id| {
            let frame = *runtime
                .backend
                .frames
                .entry(window(id))
                .or_insert_with(|| frame(600));
            (window(id), frame)
        })
        .collect::<Vec<_>>();
    runtime
        .backend
        .frames
        .retain(|window, _| windows.iter().any(|(open, _)| open == window));
    focus_on(runtime, focused);
    runtime.backend.observation = Some(observation_for_windows(
        "com.example.Test",
        &window(focused),
        &windows,
    ));
    runtime
        .cycle(landscape_outputs(), &AtomicBool::new(false))
        .unwrap();
}

fn landscape_frames(runtime: &WorkspaceRuntime<FakeBackend>, ids: &[u32]) -> Vec<crate::Rect> {
    ids.iter()
        .map(|&id| runtime.backend.frames[&window(id)])
        .collect()
}

/// Default Dwindle on a 1280×800 display, windows opened one by one:
/// `| 10/13 | 11/12 |`, 10 joins right onto the stack Dwindle made, giving
/// `| 13 | 10/11/12 |`. Closing 13 widens the stack, and Dwindle's
/// re-orientation keeps it stacked because the join made it the user's.
#[test]
fn under_dwindle_a_stack_a_tile_joins_stays_stacked_when_a_sibling_closes() {
    let outputs = landscape_outputs();
    let cancelled = AtomicBool::new(false);
    let (mut runtime, _) = open_dwindle_windows(&outputs, &[10, 11, 12], Config::default());
    // Dwindle splits the tile focused when the new window appears.
    observe_landscape(&mut runtime, &[10, 11, 12], 10);
    observe_landscape(&mut runtime, &[10, 11, 12, 13], 10);
    assert_eq!(
        landscape_frames(&runtime, &[10, 13, 11, 12]),
        [
            rect(0, 0, 640, 400),
            rect(0, 400, 640, 400),
            rect(640, 0, 640, 400),
            rect(640, 400, 640, 400),
        ]
    );

    focus_on(&mut runtime, 10);
    runtime
        .join_focused(Direction::Right, outputs, &cancelled)
        .unwrap();
    assert_eq!(
        landscape_frames(&runtime, &[13, 10, 11, 12]),
        [
            rect(0, 0, 640, 800),
            rect(640, 0, 640, 266),
            rect(640, 266, 640, 267),
            rect(640, 533, 640, 267),
        ]
    );

    observe_landscape(&mut runtime, &[10, 11, 12], 10);

    assert_eq!(
        landscape_frames(&runtime, &[10, 11, 12]),
        [
            rect(0, 0, 1280, 266),
            rect(0, 266, 1280, 267),
            rect(0, 533, 1280, 267),
        ]
    );
}

/// Default Dwindle: `| 10 | 11/12 |`, 12 joins left, `| 10/12 | 11 |`. A new
/// window opened with 12 focused splits 12's wide tile side by side, inside
/// the pair; closing 11 then widens the pair, which stays stacked.
#[test]
fn under_dwindle_a_new_window_after_a_join_splits_the_focused_tile_in_the_pair() {
    let outputs = landscape_outputs();
    let cancelled = AtomicBool::new(false);
    let (mut runtime, _) = open_dwindle_windows(&outputs, &[10, 11, 12], Config::default());
    focus_on(&mut runtime, 12);
    runtime
        .join_focused(Direction::Left, outputs, &cancelled)
        .unwrap();
    assert_eq!(
        landscape_frames(&runtime, &[10, 12, 11]),
        [
            rect(0, 0, 640, 400),
            rect(0, 400, 640, 400),
            rect(640, 0, 640, 800),
        ]
    );

    observe_landscape(&mut runtime, &[10, 11, 12, 13], 12);
    assert_eq!(
        landscape_frames(&runtime, &[10, 12, 13, 11]),
        [
            rect(0, 0, 640, 400),
            rect(0, 400, 320, 400),
            rect(320, 400, 320, 400),
            rect(640, 0, 640, 800),
        ]
    );

    observe_landscape(&mut runtime, &[10, 12, 13], 12);

    assert_eq!(
        landscape_frames(&runtime, &[10, 12, 13]),
        [
            rect(0, 0, 1280, 400),
            rect(0, 400, 640, 400),
            rect(640, 400, 640, 400),
        ]
    );
}

/// Default Dwindle: `| 10 | 11/12 |`, 12 joins left, `| 10/12 | 11 |`. The
/// pair is saved as the user's split, so after a restart closing 11 widens it
/// and it stays stacked rather than turning side by side.
#[test]
fn under_dwindle_a_join_pair_stays_stacked_across_a_restart_and_a_close() {
    let (mut runtime, _) =
        open_dwindle_windows(&landscape_outputs(), &[10, 11, 12], Config::default());
    join(&mut runtime, 12, Direction::Left);
    let mut restarted = restart_runtime(&runtime, window(10), Config::default());

    observe_landscape(&mut restarted, &[10, 12], 10);

    assert_eq!(
        landscape_frames(&restarted, &[10, 12]),
        [rect(0, 0, 1280, 400), rect(0, 400, 1280, 400)]
    );
}

/// Default Dwindle: `| 10/12 | 11 |` after a join; floating 11 re-fills its
/// space like a close, and the widened pair stays stacked.
#[test]
fn under_dwindle_a_join_pair_stays_stacked_when_a_sibling_floats() {
    let (mut runtime, _) =
        open_dwindle_windows(&landscape_outputs(), &[10, 11, 12], Config::default());
    join(&mut runtime, 12, Direction::Left);
    focus_on(&mut runtime, 11);

    runtime
        .toggle_float(landscape_outputs(), &AtomicBool::new(false))
        .unwrap();

    assert_eq!(
        landscape_frames(&runtime, &[10, 12]),
        [rect(0, 0, 1280, 400), rect(0, 400, 1280, 400)]
    );
}

/// Default Dwindle: `| 10 | 11 |`, 11 joins left, `10/11`. A window opened
/// with 11 focused splits 11's wide tile side by side, inside the rows, and
/// closing it gives the two rows back.
#[test]
fn under_dwindle_joining_the_only_other_tile_keeps_rows_as_windows_come_and_go() {
    let (mut runtime, _) = open_dwindle_windows(&landscape_outputs(), &[10, 11], Config::default());
    join(&mut runtime, 11, Direction::Left);
    let rows = [rect(0, 0, 1280, 400), rect(0, 400, 1280, 400)];
    assert_eq!(landscape_frames(&runtime, &[10, 11]), rows);

    observe_landscape(&mut runtime, &[10, 11, 12], 11);
    assert_eq!(
        landscape_frames(&runtime, &[10, 11, 12]),
        [
            rect(0, 0, 1280, 400),
            rect(0, 400, 640, 400),
            rect(640, 400, 640, 400),
        ]
    );

    observe_landscape(&mut runtime, &[10, 11], 11);
    assert_eq!(landscape_frames(&runtime, &[10, 11]), rows);
}

/// Columns, windows opened one by one: `| 10 | 11 | 12 |`; 12 joins left,
/// `| 10 | 11/12 |`; 13 opens beside 10, `| 10 | 13 | 11/12 |`; 13 joins
/// right onto the stack at its facing edge, `| 10 | 13/11/12 |`, and the
/// stack keeps that order when closing 10 widens it.
#[test]
fn under_columns_joining_a_stack_through_real_insertion_adds_at_its_facing_edge() {
    let (mut runtime, _) =
        open_dwindle_windows(&landscape_outputs(), &[10, 11, 12], columns_config());
    join(&mut runtime, 12, Direction::Left);
    observe_landscape(&mut runtime, &[10, 11, 12], 10);
    observe_landscape(&mut runtime, &[10, 11, 12, 13], 10);
    assert_eq!(
        landscape_frames(&runtime, &[10, 13, 11, 12]),
        [
            rect(0, 0, 426, 800),
            rect(426, 0, 427, 800),
            rect(853, 0, 427, 400),
            rect(853, 400, 427, 400),
        ]
    );

    join(&mut runtime, 13, Direction::Right);
    observe_landscape(&mut runtime, &[11, 12, 13], 13);

    assert_eq!(
        landscape_frames(&runtime, &[13, 11, 12]),
        [
            rect(0, 0, 1280, 266),
            rect(0, 266, 1280, 267),
            rect(0, 533, 1280, 267),
        ]
    );
}
