use crate::{Direction, SlotId};

use super::{Child, LayoutTree, Node, average_weight, direction_axis, split_pair};

impl LayoutTree {
    /// Joins `moved` with `target`, its neighbor in `direction`, in a split
    /// across that direction. A target already stacked across the direction,
    /// in a split without `moved`, takes `moved` at the stack's edge facing
    /// where `moved` came from; one already in such a split with `moved`
    /// leaves the layout alone. Otherwise the target's tile becomes the pair
    /// `[target, moved]` in reading order: the pair takes the target's share,
    /// and the space `moved` leaves goes to its former siblings.
    pub(crate) fn join(&mut self, moved: SlotId, target: SlotId, direction: Direction) -> bool {
        if moved == target || !self.root.contains(target) {
            return false;
        }
        let side = join_side(direction);
        let across = parent_of(&self.root, target).filter(
            |parent| matches!(parent, Node::Split { axis, .. } if *axis == direction_axis(side)),
        );
        // Already together across the direction: nothing to join.
        if across.is_some_and(|parent| parent.contains(moved)) {
            return false;
        }
        let stack = across.is_some();
        let Some((mut remaining, extracted)) = self.take_slot(moved) else {
            return false;
        };
        let joined = if stack {
            let at_end = matches!(direction, Direction::Left | Direction::Up);
            insert_at_edge(&mut remaining, target, extracted, at_end)
        } else {
            wrap_leaf(&mut remaining, target, extracted, side)
        };
        if joined {
            self.root = remaining;
        }
        joined
    }
}

/// The split whose children include `slot`'s leaf.
fn parent_of(node: &Node, slot: SlotId) -> Option<&Node> {
    let Node::Split { children, .. } = node else {
        return None;
    };
    if children.iter().any(|child| child.node == Node::Leaf(slot)) {
        return Some(node);
    }
    children
        .iter()
        .find_map(|child| parent_of(&child.node, slot))
}

/// Adds `moved` to the split holding `target`, first or last, and marks that
/// split as arranged by the user.
fn insert_at_edge(node: &mut Node, target: SlotId, moved: Node, at_end: bool) -> bool {
    let Node::Split {
        children,
        auto_axis,
        ..
    } = node
    else {
        return false;
    };
    if children
        .iter()
        .any(|child| child.node == Node::Leaf(target))
    {
        // The stack is now one the user arranged: Dwindle's re-orientation
        // keeps its axis, as it does for a split a move inserts into.
        *auto_axis = false;
        let index = if at_end { children.len() } else { 0 };
        let weight = average_weight(children);
        children.insert(
            index,
            Child {
                weight,
                node: moved,
            },
        );
        return true;
    }
    children
        .iter_mut()
        .find(|child| child.node.contains(target))
        .is_some_and(|child| insert_at_edge(&mut child.node, target, moved, at_end))
}

/// Replaces `target`'s leaf with the pair of it and `moved`, in the leaf's
/// place and with its weight.
fn wrap_leaf(node: &mut Node, target: SlotId, moved: Node, side: Direction) -> bool {
    match node {
        Node::Leaf(existing) if *existing == target => {
            *node = split_pair(side, moved, Node::Leaf(target));
            true
        }
        Node::Leaf(_) => false,
        Node::Split { children, .. } => children
            .iter_mut()
            .find(|child| child.node.contains(target))
            .is_some_and(|child| wrap_leaf(&mut child.node, target, moved, side)),
    }
}

/// Where a joined tile goes beside the neighbor it pairs with: across the
/// join direction, after a neighbor that came before it and before one after.
const fn join_side(direction: Direction) -> Direction {
    match direction {
        Direction::Left => Direction::Down,
        Direction::Up => Direction::Right,
        Direction::Right => Direction::Up,
        Direction::Down => Direction::Left,
    }
}
