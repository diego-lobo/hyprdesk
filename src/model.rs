//! The desk model: pure arithmetic, no I/O.
//!
//! A desk is one logical desktop spanning all monitors. Desk `d` on the
//! monitor with index `m` (monitors sorted by Hyprland id) owns Hyprland
//! workspace `d + 10*m`, so the last digit of a workspace id is the desk
//! identity and the tens digit is the monitor index. This mapping is total
//! and stateless: it survives monitor hotplug without remembered layouts,
//! keeps waybar legible, and lets `workspace = N, monitor:X` rules pin
//! every workspace to its monitor.

use std::fmt;
use std::str::FromStr;

/// Number of desks; SUPER+1..0 addresses exactly this many.
pub const DESK_COUNT: u8 = 10;

/// Workspace-id stride between monitor columns; equals `DESK_COUNT` so the
/// mapping packs with no gaps or collisions.
const MONITOR_STRIDE: i64 = DESK_COUNT as i64;

/// Upper bound on monitor slots used when inferring desks from workspace
/// ids; ids at or beyond `MONITOR_STRIDE * MAX_MONITORS` were not created
/// by hyprdesk.
pub const MAX_MONITORS: u8 = 8;

/// A validated desk identifier. Invariant: `1 <= id <= DESK_COUNT`.
/// Ordered by desk number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeskId(u8);

/// Behavior of [`DeskId::next`] / [`DeskId::prev`] at the ends of the desk
/// range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrap {
    /// Stop at desk 1 / desk `DESK_COUNT`.
    Clamp,
    /// Wrap around to the opposite end.
    Cycle,
}

impl DeskId {
    pub const FIRST: Self = Self(1);
    pub const LAST: Self = Self(DESK_COUNT);

    /// Accepts only `1..=DESK_COUNT`.
    pub fn new(id: u8) -> Option<Self> {
        (1..=DESK_COUNT).contains(&id).then_some(Self(id))
    }

    /// All desks in ascending order.
    pub fn all() -> impl Iterator<Item = Self> {
        (1..=DESK_COUNT).map(Self)
    }

    /// The workspace this desk owns on the monitor with the given index.
    ///
    /// # Panics
    /// Never in practice: `monitor_index` comes from enumerating live
    /// monitors, far below `i64::MAX / MONITOR_STRIDE`.
    pub fn workspace_on(self, monitor_index: usize) -> i64 {
        let index = i64::try_from(monitor_index).expect("monitor index fits in i64");
        i64::from(self.0) + MONITOR_STRIDE * index
    }

    /// The desk that owns the given workspace, if any. Special workspaces
    /// (negative ids) and out-of-range ids map to `None`.
    pub fn of_workspace(workspace: i64) -> Option<Self> {
        monitor_index_of(workspace)?;
        u8::try_from((workspace - 1) % MONITOR_STRIDE + 1)
            .ok()
            .and_then(Self::new)
    }

    pub fn next(self, wrap: Wrap) -> Self {
        match (self, wrap) {
            (Self::LAST, Wrap::Clamp) => Self::LAST,
            (Self::LAST, Wrap::Cycle) => Self::FIRST,
            (Self(id), _) => Self(id + 1),
        }
    }

    pub fn prev(self, wrap: Wrap) -> Self {
        match (self, wrap) {
            (Self::FIRST, Wrap::Clamp) => Self::FIRST,
            (Self::FIRST, Wrap::Cycle) => Self::LAST,
            (Self(id), _) => Self(id - 1),
        }
    }
}

/// The monitor index encoded in a workspace id, if the id belongs to the
/// desk mapping at all.
pub fn monitor_index_of(workspace: i64) -> Option<usize> {
    let max = MONITOR_STRIDE * i64::from(MAX_MONITORS);
    if !(1..=max).contains(&workspace) {
        return None;
    }
    usize::try_from((workspace - 1) / MONITOR_STRIDE).ok()
}

impl fmt::Display for DeskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Reason a string failed to parse as a [`DeskId`]; renders the valid range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidDeskId;

impl fmt::Display for InvalidDeskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "desk must be a number in 1..={DESK_COUNT}")
    }
}

impl FromStr for DeskId {
    type Err = InvalidDeskId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u8>()
            .ok()
            .and_then(Self::new)
            .ok_or(InvalidDeskId)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desk(id: u8) -> DeskId {
        DeskId::new(id).expect("valid desk id in test")
    }

    #[test]
    fn mapping_round_trips() {
        for id in 1..=DESK_COUNT {
            for m in 0..usize::from(MAX_MONITORS) {
                let ws = desk(id).workspace_on(m);
                assert_eq!(DeskId::of_workspace(ws), Some(desk(id)));
                assert_eq!(monitor_index_of(ws), Some(m));
            }
        }
    }

    #[test]
    fn special_and_foreign_workspaces_are_rejected() {
        assert_eq!(DeskId::of_workspace(-98), None); // special:scratchpad
        assert_eq!(DeskId::of_workspace(0), None);
        assert_eq!(
            DeskId::of_workspace(MONITOR_STRIDE * i64::from(MAX_MONITORS) + 1),
            None
        );
    }

    #[test]
    fn desk_ten_maps_to_round_workspace_ids() {
        assert_eq!(desk(10).workspace_on(0), 10);
        assert_eq!(desk(10).workspace_on(1), 20);
        assert_eq!(DeskId::of_workspace(20), Some(desk(10)));
        assert_eq!(monitor_index_of(20), Some(1));
    }

    #[test]
    fn stepping_clamps_and_cycles() {
        assert_eq!(DeskId::LAST.next(Wrap::Clamp), DeskId::LAST);
        assert_eq!(DeskId::LAST.next(Wrap::Cycle), DeskId::FIRST);
        assert_eq!(desk(3).next(Wrap::Cycle), desk(4));
        assert_eq!(DeskId::FIRST.prev(Wrap::Clamp), DeskId::FIRST);
        assert_eq!(DeskId::FIRST.prev(Wrap::Cycle), DeskId::LAST);
        assert_eq!(desk(3).prev(Wrap::Clamp), desk(2));
    }

    #[test]
    fn parsing_validates_range() {
        assert_eq!("3".parse::<DeskId>(), Ok(desk(3)));
        assert_eq!("10".parse::<DeskId>(), Ok(DeskId::LAST));
        assert!("0".parse::<DeskId>().is_err());
        assert!("11".parse::<DeskId>().is_err());
        assert!("junk".parse::<DeskId>().is_err());
    }
}
