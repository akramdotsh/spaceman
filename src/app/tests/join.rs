use super::*;
use crate::config::LayoutMode;

/// One column per id on workspace 1 of the main display (1000×800), in
/// order, observed once so the tiles hold their layout frames, with
/// `focused` focused.
fn columns(ids: &[u32], focused: u32) -> WorkspaceRuntime<FakeBackend> {
    columns_in(ids, focused, Config::default())
}

/// [`columns`] under `config`.
fn columns_in(ids: &[u32], focused: u32, config: Config) -> WorkspaceRuntime<FakeBackend> {
    let windows = ids
        .iter()
        .zip(0..)
        .map(|(&id, column)| (window(id), frame(50 + column * 200)))
        .collect::<Vec<_>>();
    let backend = FakeBackend {
        frames: windows.iter().cloned().collect(),
        focused: Some(window(focused)),
        frontmost: Some(window(focused).app),
        observation: Some(observation_for_windows(
            "com.example.Test",
            &window(focused),
            &windows,
        )),
        ..FakeBackend::default()
    };
    let mut runtime = runtime_with_config(backend, config);
    for (window, _) in &windows {
        bind(&mut runtime, WorkspaceId::ONE, window);
    }
    runtime.cycle(outputs(), &AtomicBool::new(false)).unwrap();
    runtime.backend.calls.clear();
    runtime.backend.workspace_state = None;
    runtime
}

/// `| A | B | C |` as windows 10, 20 and 30.
fn three_columns(focused: u32) -> WorkspaceRuntime<FakeBackend> {
    columns(&[10, 20, 30], focused)
}

fn join(
    runtime: &mut WorkspaceRuntime<FakeBackend>,
    direction: Direction,
) -> Result<CycleStatus, String> {
    runtime.join_focused(direction, outputs(), &AtomicBool::new(false))
}

pub(super) fn focus_on(runtime: &mut WorkspaceRuntime<FakeBackend>, id: u32) {
    runtime.backend.focused = Some(window(id));
    runtime.backend.frontmost = Some(window(id).app);
}

pub(super) fn rect(x: i32, y: i32, width: i32, height: i32) -> crate::Rect {
    crate::Rect {
        x,
        y,
        width,
        height,
    }
}

/// Each tile's window id and frame on workspace 1, in layout order.
fn tiles(runtime: &WorkspaceRuntime<FakeBackend>) -> Vec<(u32, crate::Rect)> {
    runtime
        .workspaces
        .engine(WorkspaceId::ONE)
        .placements(&Settings::for_output(outputs().main().frame))
        .into_iter()
        .map(|placement| (placement.window.unwrap().window_id, placement.frame))
        .collect()
}

/// Issue 250, Kit's diagram: focus C, join left, `| A | B/C |`.
#[test]
fn join_left_stacks_the_focused_tile_under_its_left_neighbor() {
    let mut runtime = three_columns(30);

    join(&mut runtime, Direction::Left).unwrap();

    let expected = [
        (10, rect(0, 0, 500, 800)),
        (20, rect(500, 0, 500, 400)),
        (30, rect(500, 400, 500, 400)),
    ];
    assert_eq!(tiles(&runtime), expected);
    for (id, frame) in expected {
        assert_eq!(runtime.backend.frames[&window(id)], frame, "window {id}");
    }
    let persisted = serde_json::to_value(runtime.backend.workspace_state.as_ref().unwrap())
        .unwrap()["layouts"][0]
        .clone();
    assert_eq!(
        persisted,
        serde_json::to_value(runtime.workspaces.engine(WorkspaceId::ONE).layout()).unwrap(),
        "the nested layout is persisted"
    );
    assert_ne!(
        persisted,
        serde_json::to_value(crate::layout_tree::LayoutTree::flat(
            crate::Axis::Horizontal,
            [SlotId(10), SlotId(20), SlotId(30)],
        ))
        .unwrap()
    );
    assert_eq!(runtime.backend.focused, Some(window(30)));
}

#[test]
fn join_right_keeps_the_focused_tile_first() {
    let mut runtime = three_columns(10);

    join(&mut runtime, Direction::Right).unwrap();

    assert_eq!(
        tiles(&runtime),
        [
            (10, rect(0, 0, 500, 400)),
            (20, rect(0, 400, 500, 400)),
            (30, rect(500, 0, 500, 800)),
        ]
    );
}

#[test]
fn joining_a_tile_already_in_a_stack_adds_to_that_stack() {
    let mut runtime = three_columns(30);
    join(&mut runtime, Direction::Left).unwrap();
    focus_on(&mut runtime, 10);

    join(&mut runtime, Direction::Right).unwrap();

    assert_eq!(
        tiles(&runtime),
        [
            (10, rect(0, 0, 1000, 266)),
            (20, rect(0, 266, 1000, 267)),
            (30, rect(0, 533, 1000, 267)),
        ]
    );
    assert_eq!(
        runtime.workspaces.engine(WorkspaceId::ONE).root_axis(),
        crate::Axis::Vertical
    );
}

/// A window opened after a join lands inside the joined split in both
/// layout modes: Columns adds it to the stack under the focused tile, and
/// Dwindle splits the focused tile along its longer side.
#[test]
fn a_new_window_opened_after_a_join_joins_the_nested_split() {
    for mode in [LayoutMode::Columns, LayoutMode::Dwindle] {
        let config = match mode {
            LayoutMode::Dwindle => Config::default(),
            LayoutMode::Columns => {
                Config::parse("version = 1\n[settings]\nlayout = \"columns\"\n").unwrap()
            }
        };
        let mut runtime = columns_in(&[10, 20, 30], 30, config);
        join(&mut runtime, Direction::Left).unwrap();
        let mut windows = [10, 20, 30]
            .map(|id| (window(id), runtime.backend.frames[&window(id)]))
            .to_vec();
        windows.push((window(40), frame(600)));
        runtime.backend.frames.insert(window(40), frame(600));
        runtime.backend.observation = Some(observation_for_windows(
            "com.example.Test",
            &window(30),
            &windows,
        ));

        runtime.cycle(outputs(), &AtomicBool::new(false)).unwrap();

        let expected = match mode {
            LayoutMode::Columns => [
                (10, rect(0, 0, 500, 800)),
                (20, rect(500, 0, 500, 266)),
                (30, rect(500, 266, 500, 267)),
                (40, rect(500, 533, 500, 267)),
            ],
            LayoutMode::Dwindle => [
                (10, rect(0, 0, 500, 800)),
                (20, rect(500, 0, 500, 400)),
                (30, rect(500, 400, 250, 400)),
                (40, rect(750, 400, 250, 400)),
            ],
        };
        assert_eq!(tiles(&runtime), expected, "{mode:?}");
    }
}

#[test]
fn join_without_a_neighbor_that_way_changes_nothing() {
    let mut runtime = three_columns(10);
    let before = tiles(&runtime);

    for direction in [Direction::Left, Direction::Up, Direction::Down] {
        assert_eq!(join(&mut runtime, direction).unwrap().failed, 0);
    }

    assert_eq!(tiles(&runtime), before);
    assert!(runtime.backend.workspace_state.is_none());
    assert!(
        runtime.backend.calls.is_empty(),
        "{:?}",
        runtime.backend.calls
    );
}

#[test]
fn a_single_tile_has_nothing_to_join() {
    let mut runtime = columns(&[10], 10);
    let before = tiles(&runtime);
    assert_eq!(before, [(10, outputs().main().frame)]);

    for direction in [
        Direction::Left,
        Direction::Down,
        Direction::Up,
        Direction::Right,
    ] {
        assert_eq!(join(&mut runtime, direction).unwrap().failed, 0);
    }

    assert_eq!(tiles(&runtime), before);
    assert!(
        runtime.backend.calls.is_empty(),
        "{:?}",
        runtime.backend.calls
    );
    assert!(runtime.backend.workspace_state.is_none());
}

/// Unlike `move-direction`, join never reaches the adjacent display.
#[test]
fn join_does_not_cross_to_the_adjacent_display() {
    let mut runtime = three_columns(30);
    let other = window(60);
    runtime.backend.frames.insert(other.clone(), frame(1100));
    bind(&mut runtime, WorkspaceId::SIX, &other);
    assert!(runtime.is_visible(WorkspaceId::SIX, &outputs()));
    let before = tiles(&runtime);

    join(&mut runtime, Direction::Right).unwrap();

    assert_eq!(tiles(&runtime), before);
    assert_eq!(
        runtime.workspaces.workspace_of(&window(30)),
        Some(WorkspaceId::ONE)
    );
    assert_eq!(
        runtime.workspaces.engine(WorkspaceId::SIX).bound_windows(),
        [other]
    );
}

#[test]
fn join_is_refused_in_fullscreen_layout() {
    let mut runtime = three_columns(30);
    runtime
        .toggle_fullscreen_layout(outputs(), &AtomicBool::new(false))
        .unwrap();

    let result = join(&mut runtime, Direction::Left);

    assert_eq!(
        result.err().as_deref(),
        Some("Exit Fullscreen Layout before joining tiles in the split layout")
    );
    runtime
        .toggle_fullscreen_layout(outputs(), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        tiles(&runtime).iter().map(|(_, f)| f.x).collect::<Vec<_>>(),
        [0, 333, 666]
    );
}

#[test]
fn join_needs_a_tiled_window() {
    let mut runtime = three_columns(30);
    runtime
        .toggle_float(outputs(), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        runtime.workspaces.floating_workspace_of(&window(30)),
        Some(WorkspaceId::ONE)
    );
    let before = tiles(&runtime);

    let result = join(&mut runtime, Direction::Left);

    assert_eq!(
        result.err().as_deref(),
        Some("Focused window is not managed by Spaceman")
    );
    assert_eq!(tiles(&runtime), before);
}

#[test]
fn join_up_and_down_pair_rows_across_a_vertical_root() {
    let rows = |focused| {
        let mut runtime = three_columns(focused);
        runtime
            .toggle_layout(outputs(), &AtomicBool::new(false))
            .unwrap();
        runtime
    };
    let mut up = rows(30);
    join(&mut up, Direction::Up).unwrap();
    assert_eq!(
        tiles(&up),
        [
            (10, rect(0, 0, 1000, 400)),
            (20, rect(0, 400, 500, 400)),
            (30, rect(500, 400, 500, 400)),
        ]
    );

    let mut down = rows(10);
    join(&mut down, Direction::Down).unwrap();
    assert_eq!(
        tiles(&down),
        [
            (10, rect(0, 0, 500, 400)),
            (20, rect(500, 0, 500, 400)),
            (30, rect(0, 400, 1000, 400)),
        ]
    );
}

/// `| A | D | B/C |`, focus D, join left: `| A/D | B/C |`, as halves.
#[test]
fn joining_beside_a_stack_pairs_with_the_neighbor_only() {
    let mut runtime = columns(&[10, 40, 20, 30], 30);
    join(&mut runtime, Direction::Left).unwrap();
    assert_eq!(
        tiles(&runtime)
            .iter()
            .map(|(_, f)| f.width)
            .collect::<Vec<_>>(),
        [333, 333, 334, 334],
        "| A | D | B/C | in equal thirds"
    );
    focus_on(&mut runtime, 40);

    join(&mut runtime, Direction::Left).unwrap();

    assert_eq!(
        tiles(&runtime),
        [
            (10, rect(0, 0, 500, 400)),
            (40, rect(0, 400, 500, 400)),
            (20, rect(500, 0, 500, 400)),
            (30, rect(500, 400, 500, 400)),
        ]
    );
}

/// `| A | B/C |`: B joins A as a pair, then C joins the A/B stack at its end.
#[test]
fn a_stacked_tile_pairs_with_its_neighbor_and_the_next_one_joins_the_stack() {
    let mut runtime = three_columns(30);
    join(&mut runtime, Direction::Left).unwrap();
    focus_on(&mut runtime, 20);

    join(&mut runtime, Direction::Left).unwrap();
    assert_eq!(
        tiles(&runtime),
        [
            (10, rect(0, 0, 500, 400)),
            (20, rect(0, 400, 500, 400)),
            (30, rect(500, 0, 500, 800)),
        ]
    );

    focus_on(&mut runtime, 30);
    join(&mut runtime, Direction::Left).unwrap();
    assert_eq!(
        tiles(&runtime),
        [
            (10, rect(0, 0, 1000, 266)),
            (20, rect(0, 266, 1000, 267)),
            (30, rect(0, 533, 1000, 267)),
        ]
    );
}

/// `| A | B/C |`, focus C, join left: `| A/C | B |`.
#[test]
fn the_bottom_of_a_stack_pairs_with_its_left_neighbor() {
    let mut runtime = three_columns(30);
    join(&mut runtime, Direction::Left).unwrap();

    join(&mut runtime, Direction::Left).unwrap();

    assert_eq!(
        tiles(&runtime),
        [
            (10, rect(0, 0, 500, 400)),
            (30, rect(0, 400, 500, 400)),
            (20, rect(500, 0, 500, 800)),
        ]
    );
}

/// `| A | B | C | D |`: C then D join left, giving `| A | B/C/D |`. D's
/// neighbor is B, the stack's top, yet D lands below C.
#[test]
fn successive_joins_build_a_stack_in_order() {
    let mut runtime = columns(&[10, 20, 30, 40], 30);
    join(&mut runtime, Direction::Left).unwrap();
    focus_on(&mut runtime, 40);
    let shown = runtime.navigation_tiles(&outputs());
    assert_eq!(
        crate::navigation::target(&shown, &window(40), Direction::Left)
            .map(|tile| tile.window.window_id),
        Some(20),
        "D's neighbor is the top of the stack"
    );

    join(&mut runtime, Direction::Left).unwrap();

    assert_eq!(
        tiles(&runtime),
        [
            (10, rect(0, 0, 500, 800)),
            (20, rect(500, 0, 500, 266)),
            (30, rect(500, 266, 500, 267)),
            (40, rect(500, 533, 500, 267)),
        ]
    );
}

/// `| A | B/C |` with C grown, so A's right neighbor is C, not B: A still
/// joins the stack on top.
#[test]
fn joining_a_resized_stack_keeps_the_stack_order() {
    let mut runtime = three_columns(30);
    join(&mut runtime, Direction::Left).unwrap();
    runtime
        .resize_focused(ResizeDirection::Grow, outputs(), &AtomicBool::new(false))
        .unwrap();
    let local = runtime.navigation_tiles(&outputs());
    assert_eq!(
        crate::navigation::target(&local, &window(10), Direction::Right)
            .map(|tile| tile.window.window_id),
        Some(30),
        "the grown tile is A's neighbor"
    );
    focus_on(&mut runtime, 10);

    join(&mut runtime, Direction::Right).unwrap();

    assert_eq!(
        tiles(&runtime)
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
        [10, 20, 30]
    );
    assert_eq!(
        runtime.workspaces.engine(WorkspaceId::ONE).root_axis(),
        crate::Axis::Vertical
    );
    assert!(
        tiles(&runtime)
            .iter()
            .all(|(_, f)| f.x == 0 && f.width == 1000)
    );
}

/// Rows `A / (B|C)`, focus A, join down: A joins the B|C row at its start.
#[test]
fn joining_a_row_on_a_vertical_root_adds_at_its_facing_edge() {
    let mut runtime = three_columns(30);
    runtime
        .toggle_layout(outputs(), &AtomicBool::new(false))
        .unwrap();
    join(&mut runtime, Direction::Up).unwrap();
    focus_on(&mut runtime, 10);

    join(&mut runtime, Direction::Down).unwrap();

    assert_eq!(
        tiles(&runtime),
        [
            (10, rect(0, 0, 333, 800)),
            (20, rect(333, 0, 333, 800)),
            (30, rect(666, 0, 334, 800)),
        ]
    );
    assert_eq!(
        runtime.workspaces.engine(WorkspaceId::ONE).root_axis(),
        crate::Axis::Horizontal
    );
}

#[test]
fn a_joined_layout_survives_a_state_reload() {
    let mut runtime = three_columns(30);
    join(&mut runtime, Direction::Left).unwrap();
    let joined = tiles(&runtime);
    let backend = FakeBackend {
        frames: runtime.backend.frames.clone(),
        focused: Some(window(30)),
        ..FakeBackend::default()
    };

    let mut restored = WorkspaceRuntime::with_backend_config_state(
        ManagementScope::Both,
        backend,
        HashMap::new(),
        Config::default(),
        Some(runtime.backend.workspace_state.clone().unwrap()),
    );
    restored.layout_workspace(
        WorkspaceId::ONE,
        outputs().main().frame,
        &AtomicBool::new(false),
    );

    assert_eq!(tiles(&restored), joined);
    for (id, frame) in joined {
        assert_eq!(restored.backend.frames[&window(id)], frame, "window {id}");
    }
}

/// `| A | B | C |` with minimum widths that make the display show
/// `| A | B/C |`: tiles stacked only on screen are already together.
fn temporarily_stacked(focused: u32) -> WorkspaceRuntime<FakeBackend> {
    let mut backend = FakeBackend {
        focused: Some(window(focused)),
        ..FakeBackend::default()
    };
    for (id, minimum) in [(10, 400), (20, 250), (30, 450)] {
        backend.frames.insert(window(id), frame(0));
        backend.minimum_widths.insert(window(id), minimum);
    }
    let mut runtime = runtime(backend);
    for id in [10, 20, 30] {
        bind(&mut runtime, WorkspaceId::ONE, &window(id));
    }
    for _ in 0..2 {
        runtime.layout_workspace(
            WorkspaceId::ONE,
            outputs().main().frame,
            &AtomicBool::new(false),
        );
    }
    let frames = &runtime.backend.frames;
    assert_eq!(
        frames[&window(20)].x,
        frames[&window(30)].x,
        "starts stacked"
    );
    runtime.backend.calls.clear();
    runtime.backend.workspace_state = None;
    runtime
}

#[test]
fn joining_tiles_a_fit_stacks_changes_nothing() {
    for (focused, direction) in [(30, Direction::Up), (20, Direction::Down)] {
        let mut runtime = temporarily_stacked(focused);
        let frames = runtime.backend.frames.clone();
        let requested = runtime.workspaces.engine(WorkspaceId::ONE).layout().clone();

        join(&mut runtime, direction).unwrap();

        assert_eq!(runtime.backend.frames, frames, "{direction:?}");
        assert!(runtime.backend.workspace_state.is_none(), "{direction:?}");
        assert_eq!(
            runtime.workspaces.engine(WorkspaceId::ONE).layout(),
            &requested
        );
    }
}

/// Two tiles a fit stacks: joining them would only reset their weights.
#[test]
fn joining_a_two_tile_fit_stack_changes_nothing() {
    let mut backend = FakeBackend {
        focused: Some(window(11)),
        ..FakeBackend::default()
    };
    for id in [10, 11] {
        backend.frames.insert(window(id), frame(0));
        backend.minimum_widths.insert(window(id), 600);
    }
    let mut runtime = runtime(backend);
    for id in [10, 11] {
        bind(&mut runtime, WorkspaceId::ONE, &window(id));
    }
    for _ in 0..2 {
        runtime.layout_workspace(
            WorkspaceId::ONE,
            outputs().main().frame,
            &AtomicBool::new(false),
        );
    }
    assert_eq!(
        runtime.backend.frames[&window(10)].x,
        runtime.backend.frames[&window(11)].x,
        "starts stacked"
    );
    runtime.backend.workspace_state = None;
    let frames = runtime.backend.frames.clone();

    join(&mut runtime, Direction::Up).unwrap();

    assert_eq!(runtime.backend.frames, frames);
    // The assertion that matters: a join here would rebuild an identical
    // row, so only the missing persist shows that nothing was joined.
    assert!(runtime.backend.workspace_state.is_none());
    assert_eq!(
        runtime
            .workspaces
            .engine(WorkspaceId::ONE)
            .layout()
            .root_axis(),
        crate::Axis::Horizontal
    );
}

/// `columns(ids)` nested by `joins` (each focused id joins that way), then
/// given these minimum widths and laid out until the display shows the fit.
fn nested_under_a_fit(
    ids: &[u32],
    joins: &[(u32, Direction)],
    minimums: &[i32],
) -> WorkspaceRuntime<FakeBackend> {
    let mut runtime = columns(ids, ids[0]);
    for &(id, direction) in joins {
        focus_on(&mut runtime, id);
        join(&mut runtime, direction).unwrap();
    }
    for (&id, &minimum) in ids.iter().zip(minimums) {
        runtime.backend.minimum_widths.insert(window(id), minimum);
    }
    for _ in 0..2 {
        runtime.layout_workspace(
            WorkspaceId::ONE,
            outputs().main().frame,
            &AtomicBool::new(false),
        );
    }
    runtime.backend.calls.clear();
    runtime.backend.workspace_state = None;
    runtime
}

/// Joins `id` toward `direction` where the fit shows `neighbor` that way but
/// the requested layout does not, and checks that nothing changes.
fn join_shown_only_neighbor(
    runtime: &mut WorkspaceRuntime<FakeBackend>,
    id: u32,
    direction: Direction,
    neighbor: u32,
) {
    focus_on(runtime, id);
    let shown = runtime.navigation_tiles(&outputs());
    assert_eq!(
        crate::navigation::target(&shown, &window(id), direction).map(|tile| tile.window.window_id),
        Some(neighbor),
        "{id} {direction}: the fit shows {neighbor} that way"
    );
    let frames = runtime.backend.frames.clone();
    let requested = runtime.workspaces.engine(WorkspaceId::ONE).layout().clone();

    join(runtime, direction).unwrap();

    assert_eq!(
        runtime.workspaces.engine(WorkspaceId::ONE).layout(),
        &requested,
        "{id} {direction}"
    );
    assert_eq!(runtime.backend.frames, frames, "{id} {direction}");
    assert!(
        runtime.backend.workspace_state.is_none(),
        "{id} {direction}"
    );
}

/// `| T | M/N |` shown as `T / M / N`: joining across the fit's stack from
/// either side changes nothing.
#[test]
fn joins_across_a_fit_stack_of_a_tile_and_a_column_change_nothing() {
    let mut runtime = nested_under_a_fit(&[10, 20, 30], &[(30, Direction::Left)], &[600; 3]);
    let frames = &runtime.backend.frames;
    assert!(
        [20, 30]
            .iter()
            .all(|&id| frames[&window(id)].x == frames[&window(10)].x)
    );

    join_shown_only_neighbor(&mut runtime, 20, Direction::Up, 10);
    join_shown_only_neighbor(&mut runtime, 10, Direction::Down, 20);
}

/// `| P/Q | M |` shown as `P / Q / M`: M has no tile above it in the
/// requested layout.
#[test]
fn join_toward_a_column_the_fit_stacks_above_changes_nothing() {
    let mut runtime = nested_under_a_fit(&[10, 20, 30], &[(20, Direction::Left)], &[600; 3]);
    let frames = &runtime.backend.frames;
    assert!(
        [20, 30]
            .iter()
            .all(|&id| frames[&window(id)].x == frames[&window(10)].x)
    );

    join_shown_only_neighbor(&mut runtime, 30, Direction::Up, 20);
}

/// `| A | B/C | D |` shown as `| A | B/C/D |`: D has no tile above it in the
/// requested layout.
#[test]
fn join_into_a_column_only_the_fit_stacks_changes_nothing() {
    let mut runtime = nested_under_a_fit(
        &[10, 20, 30, 40],
        &[(30, Direction::Left)],
        &[200, 550, 550, 500],
    );
    let frames = &runtime.backend.frames;
    assert_eq!(
        frames[&window(40)].x,
        frames[&window(30)].x,
        "D stacked under B/C"
    );
    assert!(frames[&window(40)].y > frames[&window(30)].y);

    join_shown_only_neighbor(&mut runtime, 40, Direction::Up, 30);
}

/// `| A | B/C | D |` shown as `| A | B/C/D |`: the fit squeezes B and C, so
/// A's neighbor on screen is C while B would win the tie in the saved
/// layout. C is next to A there too, so the join goes ahead.
#[test]
fn join_under_a_fit_accepts_any_saved_neighbor_of_a_stack() {
    let mut runtime = nested_under_a_fit(
        &[10, 20, 30, 40],
        &[(30, Direction::Left)],
        &[200, 550, 550, 500],
    );
    focus_on(&mut runtime, 10);
    let shown = runtime.navigation_tiles(&outputs());
    assert_eq!(
        crate::navigation::target(&shown, &window(10), Direction::Right)
            .map(|tile| tile.window.window_id),
        Some(30),
        "the fit shows C as A's neighbor"
    );

    join(&mut runtime, Direction::Right).unwrap();

    let requested = runtime
        .workspaces
        .engine(WorkspaceId::ONE)
        .layout()
        .placements(outputs().main().frame)
        .into_iter()
        .map(|(slot, frame)| (slot.0 as u32, frame))
        .collect::<Vec<_>>();
    assert_eq!(
        requested,
        [
            (10, rect(0, 0, 500, 266)),
            (20, rect(0, 266, 500, 267)),
            (30, rect(0, 533, 500, 267)),
            (40, rect(500, 0, 500, 800)),
        ]
    );
    assert!(runtime.backend.workspace_state.is_some());
}

fn act(runtime: &mut WorkspaceRuntime<FakeBackend>, action: WorkerAction) {
    let Some(WorkerResult::Status { result, .. }) = runtime.execute(
        WorkerCommand::Action(action, outputs()),
        &AtomicBool::new(false),
    ) else {
        panic!("missing status");
    };
    result.unwrap();
}

/// Each tile's window id and origin on workspace 1, in layout order.
fn origins(runtime: &WorkspaceRuntime<FakeBackend>) -> Vec<(u32, i32, i32)> {
    tiles(runtime)
        .into_iter()
        .map(|(id, frame)| (id, frame.x, frame.y))
        .collect()
}

/// `| 10 | 20 | 30 | 40 |` with 10 focused; Option+L asks for 20, but
/// Accessibility still reports `reported` when join right runs.
fn join_right_while_focus_lags(
    reported: Option<u32>,
    lag: Duration,
) -> WorkspaceRuntime<FakeBackend> {
    let mut runtime = columns(&[10, 20, 30, 40], 10);
    act(&mut runtime, WorkerAction::Focus(Direction::Right));
    assert_eq!(runtime.backend.focused, Some(window(20)));
    runtime.backend.focused = reported.map(window);
    runtime.backend.advance(lag);

    act(&mut runtime, WorkerAction::Join(Direction::Right));
    runtime
}

/// Join right after Option+L, before Accessibility reports the new focus
/// (or while it reports none): join acts on the tile the focus request is
/// for, as move-direction and Option+H/J/K/L do, so 20 pairs with 30.
#[test]
fn join_acts_on_an_in_flight_focus_request() {
    for reported in [Some(10), None] {
        let runtime = join_right_while_focus_lags(reported, Duration::ZERO);

        assert_eq!(
            origins(&runtime),
            [(10, 0, 0), (20, 333, 0), (30, 333, 400), (40, 666, 0)],
            "reported {reported:?}"
        );
    }
}

/// Once the focus request has outlived its settling time on the worker's
/// clock, the reported focus wins again: 10 pairs with 20.
#[test]
fn join_falls_back_to_the_reported_focus_after_the_focus_timeout() {
    let runtime = join_right_while_focus_lags(Some(10), Duration::from_millis(300));

    assert_eq!(
        origins(&runtime),
        [(10, 0, 0), (20, 0, 400), (30, 333, 0), (40, 666, 0)]
    );
}
