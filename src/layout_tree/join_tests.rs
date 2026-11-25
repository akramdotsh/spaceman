use crate::{Axis, Direction, Rect, ResizeDirection, SlotId, config::LayoutMode};

use super::LayoutTree;

fn output() -> Rect {
    Rect {
        width: 1000,
        height: 800,
        ..Rect::default()
    }
}

fn tile(slot: u64, x: i32, y: i32, width: i32, height: i32) -> (SlotId, Rect) {
    (
        SlotId(slot),
        Rect {
            x,
            y,
            width,
            height,
        },
    )
}

/// Kit's diagram in issue 250: `| A | B | C |`, join C left, `| A | B/C |`.
#[test]
fn join_nests_the_tile_across_the_direction_in_reading_order() {
    let wide = Rect {
        width: 1200,
        height: 800,
        ..Rect::default()
    };
    let mut left = LayoutTree::flat(Axis::Horizontal, [SlotId(1), SlotId(2), SlotId(3)]);
    assert!(left.join(SlotId(3), SlotId(2), Direction::Left));
    assert_eq!(
        left.placements(wide),
        [
            tile(1, 0, 0, 600, 800),
            tile(2, 600, 0, 600, 400),
            tile(3, 600, 400, 600, 400),
        ]
    );

    let mut right = LayoutTree::flat(Axis::Horizontal, [SlotId(1), SlotId(2), SlotId(3)]);
    assert!(right.join(SlotId(1), SlotId(2), Direction::Right));
    assert_eq!(
        right.placements(wide),
        [
            tile(1, 0, 0, 600, 400),
            tile(2, 0, 400, 600, 400),
            tile(3, 600, 0, 600, 800),
        ]
    );

    let mut up = LayoutTree::flat(Axis::Vertical, [SlotId(1), SlotId(2), SlotId(3)]);
    assert!(up.join(SlotId(3), SlotId(2), Direction::Up));
    assert_eq!(
        up.placements(wide),
        [
            tile(1, 0, 0, 1200, 400),
            tile(2, 0, 400, 600, 400),
            tile(3, 600, 400, 600, 400),
        ]
    );

    let mut down = LayoutTree::flat(Axis::Vertical, [SlotId(1), SlotId(2), SlotId(3)]);
    assert!(down.join(SlotId(1), SlotId(2), Direction::Down));
    assert_eq!(
        down.placements(wide),
        [
            tile(1, 0, 0, 600, 400),
            tile(2, 600, 0, 600, 400),
            tile(3, 0, 400, 1200, 400),
        ]
    );
}

#[test]
fn join_into_a_split_across_the_direction_adds_a_sibling_instead_of_nesting() {
    let mut tree = LayoutTree::flat(Axis::Horizontal, [SlotId(1), SlotId(2), SlotId(3)]);
    assert!(tree.join(SlotId(3), SlotId(2), Direction::Left));

    assert!(tree.join(SlotId(1), SlotId(2), Direction::Right));

    assert_eq!(
        tree,
        LayoutTree::flat(Axis::Vertical, [SlotId(1), SlotId(2), SlotId(3)])
    );
}

#[test]
fn joining_the_only_other_tile_turns_the_root_across() {
    let mut tree = LayoutTree::flat(Axis::Horizontal, [SlotId(1), SlotId(2)]);
    assert!(!tree.join(SlotId(2), SlotId(2), Direction::Left));

    assert!(tree.join(SlotId(2), SlotId(1), Direction::Left));

    assert_eq!(
        tree,
        LayoutTree::flat(Axis::Vertical, [SlotId(1), SlotId(2)])
    );
    assert_eq!(tree.root_axis(), Axis::Vertical);
}

/// Joining a tile beside a stack in its own column pairs it with the
/// neighbor where the neighbor sits; the other column keeps its width.
#[test]
fn a_join_pair_keeps_the_neighbors_share_and_leaves_other_tiles_alone() {
    let mut tree = LayoutTree::flat(Axis::Horizontal, [SlotId(1), SlotId(2), SlotId(3)]);
    assert!(tree.join(SlotId(3), SlotId(2), Direction::Left));

    assert!(tree.join(SlotId(3), SlotId(2), Direction::Up));

    assert_eq!(
        tree.placements(output()),
        [
            tile(1, 0, 0, 500, 800),
            tile(2, 500, 0, 250, 800),
            tile(3, 750, 0, 250, 800),
        ]
    );
}

/// Issue 250 review: the joined tile goes to the stack edge facing where it
/// came from, whichever stacked tile is the neighbor.
#[test]
fn joining_a_stack_adds_at_the_edge_facing_the_tile() {
    let slots = [SlotId(1), SlotId(2), SlotId(3), SlotId(4)];
    let mut tree = LayoutTree::flat(Axis::Horizontal, slots);
    assert!(tree.join(SlotId(3), SlotId(2), Direction::Left));

    // Neighbor 2 is the stack's top: 4 still lands below 3.
    assert!(tree.join(SlotId(4), SlotId(2), Direction::Left));
    assert_eq!(tree.slots(), slots);
    let stacked = tree.placements(output());
    assert_eq!(stacked[0], tile(1, 0, 0, 500, 800));
    assert!(stacked[1..].iter().all(|(_, frame)| frame.x == 500));

    // Neighbor 4 is the stack's bottom: 1 still lands on top.
    assert!(tree.join(SlotId(1), SlotId(4), Direction::Right));
    assert_eq!(tree, LayoutTree::flat(Axis::Vertical, slots));
}

/// Resized splits hold pixel-scale weights: the pair takes exactly the
/// neighbor's weight, and the joined tile's space goes back to its old split.
#[test]
fn a_join_pair_takes_the_neighbors_pixel_weight() {
    let mut tree = LayoutTree::flat(Axis::Horizontal, [SlotId(1), SlotId(2), SlotId(3)]);
    assert!(tree.resize_by(SlotId(1), ResizeDirection::Grow, 200, output()));
    let first = u64::from(tree.weight(SlotId(1)).unwrap());
    let second = u64::from(tree.weight(SlotId(2)).unwrap());
    assert_ne!(first, second);

    assert!(tree.join(SlotId(3), SlotId(2), Direction::Left));

    let placements = tree.placements(output());
    let width = (1000 * first / (first + second)) as i32;
    assert_eq!(placements[0], tile(1, 0, 0, width, 800));
    assert_eq!(placements[1], tile(2, width, 0, 1000 - width, 400));
    assert_eq!(placements[2], tile(3, width, 400, 1000 - width, 400));
}

#[test]
fn joining_two_resized_tiles_gives_equal_halves() {
    let mut tree = LayoutTree::flat(Axis::Horizontal, [SlotId(1), SlotId(2)]);
    assert!(tree.resize_by(SlotId(1), ResizeDirection::Grow, 50, output()));

    assert!(tree.join(SlotId(2), SlotId(1), Direction::Left));

    assert_eq!(
        tree,
        LayoutTree::flat(Axis::Vertical, [SlotId(1), SlotId(2)])
    );
}

#[test]
fn join_leaves_tiles_already_together_across_the_direction_alone() {
    let mut tree = LayoutTree::flat(Axis::Horizontal, [SlotId(1), SlotId(2), SlotId(3)]);
    let before = tree.clone();

    assert!(!tree.join(SlotId(3), SlotId(2), Direction::Up));
    assert!(!tree.join(SlotId(2), SlotId(3), Direction::Down));

    assert_eq!(tree, before);
}

/// Only the neighbor's own split counts as its stack: in
/// `| A | B/(C|E) | D |`, D joining left onto E pairs with E inside the
/// bottom row, `B/(C | E/D)`, rather than joining the outer column.
#[test]
fn join_pairs_with_a_neighbor_whose_own_split_runs_along_the_direction() {
    let [a, b, c, d, e] = [1, 2, 3, 4, 5].map(SlotId);
    let mut tree = LayoutTree::flat(Axis::Horizontal, [a, b, c, e, d]);
    assert!(tree.move_relative(b, c, Direction::Up));
    assert!(tree.move_relative(e, c, Direction::Right));
    assert_eq!(
        tree.placements(output()),
        [
            tile(1, 0, 0, 333, 800),
            tile(2, 333, 0, 333, 400),
            tile(3, 333, 400, 166, 400),
            tile(5, 499, 400, 167, 400),
            tile(4, 666, 0, 334, 800),
        ]
    );

    assert!(tree.join(d, e, Direction::Left));

    assert_eq!(
        tree.placements(output()),
        [
            tile(1, 0, 0, 500, 800),
            tile(2, 500, 0, 500, 400),
            tile(3, 500, 400, 250, 400),
            tile(5, 750, 400, 250, 200),
            tile(4, 750, 600, 250, 200),
        ]
    );
}

fn dwindle_slots(ids: &[u64]) -> Vec<crate::Slot> {
    ids.iter()
        .map(|&id| crate::Slot {
            id: SlotId(id),
            binding: crate::BindingState::Bound(crate::PhysicalWindowId {
                app: crate::AppInstanceId {
                    pid: 1,
                    launched_at: 1,
                },
                window_id: id as u32,
            }),
        })
        .collect()
}

/// Dwindle opens `ids` in order on a 1280×800 display, each new tile
/// splitting the tile its `anchor` names.
fn dwindle(opened: &[(u64, Option<u64>)]) -> LayoutTree {
    let mut tree = LayoutTree::default();
    for index in 0..opened.len() {
        let ids = opened[..=index]
            .iter()
            .map(|&(id, _)| id)
            .collect::<Vec<_>>();
        assert!(tree.sync_near(
            Axis::Horizontal,
            &dwindle_slots(&ids),
            opened[index].1.map(SlotId),
            LayoutMode::Dwindle,
            wide(),
        ));
    }
    tree
}

fn wide() -> Rect {
    Rect {
        width: 1280,
        height: 800,
        ..Rect::default()
    }
}

fn close(tree: &mut LayoutTree, remaining: &[u64]) {
    assert!(tree.sync_near(
        Axis::Horizontal,
        &dwindle_slots(remaining),
        None,
        LayoutMode::Dwindle,
        wide(),
    ));
}

/// Dwindle re-orients only the splits it chose itself; a join pair is one the
/// user chose, so closing a tile never turns it back side by side.
#[test]
fn a_join_pair_keeps_its_axis_when_dwindle_re_orients_after_a_close() {
    // | 1 | 2/3 |, 3 joins left: | 1/3 | 2 |.
    let mut tree = dwindle(&[(1, None), (2, Some(1)), (3, Some(2))]);
    assert!(tree.join(SlotId(3), SlotId(1), Direction::Left));
    assert_eq!(
        tree.placements(wide()),
        [
            tile(1, 0, 0, 640, 400),
            tile(3, 0, 400, 640, 400),
            tile(2, 640, 0, 640, 800),
        ]
    );

    close(&mut tree, &[1, 3]);

    assert_eq!(
        tree.placements(wide()),
        [tile(1, 0, 0, 1280, 400), tile(3, 0, 400, 1280, 400)]
    );
}

/// A tile joining a stack Dwindle made makes it the user's: a later close
/// that widens the stack leaves it stacked, as after a move into it.
#[test]
fn a_dwindle_stack_a_tile_joins_keeps_its_axis_when_a_sibling_closes() {
    // | 1/4 | 2/3 |, 1 joins right: | 4 | 1/2/3 |.
    let mut tree = dwindle(&[(1, None), (2, Some(1)), (3, Some(2)), (4, Some(1))]);
    assert_eq!(
        tree.placements(wide()),
        [
            tile(1, 0, 0, 640, 400),
            tile(4, 0, 400, 640, 400),
            tile(2, 640, 0, 640, 400),
            tile(3, 640, 400, 640, 400),
        ]
    );
    assert!(tree.join(SlotId(1), SlotId(2), Direction::Right));
    assert_eq!(
        tree.placements(wide()),
        [
            tile(4, 0, 0, 640, 800),
            tile(1, 640, 0, 640, 266),
            tile(2, 640, 266, 640, 267),
            tile(3, 640, 533, 640, 267),
        ]
    );

    close(&mut tree, &[1, 2, 3]);

    assert_eq!(
        tree.placements(wide()),
        [
            tile(1, 0, 0, 1280, 266),
            tile(2, 0, 266, 1280, 267),
            tile(3, 0, 533, 1280, 267),
        ]
    );
}

/// Join re-orients nothing it leaves behind, like a move: under Dwindle,
/// `| 1 | 2/(3|4) |` with 2 joining left leaves 3 and 4 side by side in the
/// tall column they get, where closing 2 would have stacked them.
#[test]
fn join_leaves_the_dwindle_split_it_vacates_as_it_was() {
    let mut tree = dwindle(&[(1, None), (2, Some(1)), (3, Some(2)), (4, Some(3))]);
    let mut closed = tree.clone();

    assert!(tree.join(SlotId(2), SlotId(1), Direction::Left));
    close(&mut closed, &[1, 3, 4]);

    assert_eq!(
        tree.placements(wide()),
        [
            tile(1, 0, 0, 640, 400),
            tile(2, 0, 400, 640, 400),
            tile(3, 640, 0, 320, 800),
            tile(4, 960, 0, 320, 800),
        ]
    );
    assert_eq!(
        closed.placements(wide()),
        [
            tile(1, 0, 0, 640, 800),
            tile(3, 640, 0, 640, 400),
            tile(4, 640, 400, 640, 400),
        ]
    );
}
