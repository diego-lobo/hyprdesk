//! Rendering for waybar's `custom/hyprdesk` module.
//!
//! This is the bar path for plain-Hyprland setups. Omarchy 4 replaced
//! waybar with a Quickshell bar, so there the desk strip is drawn by
//! `config/omarchy/plugins/hyprdesk.desks/`, which derives the same view
//! from compositor state and never reads this stream. Both renderings
//! deliberately agree on the visual rules below.
//!
//! The daemon streams one JSON object per line to `hyprdesk waybar`
//! subscribers; waybar consumes it as a continuous custom module
//! (`return-type: json`). The strip mirrors Omarchy's stock workspace
//! look, lifted to desk level: desks 1..=[`PERSISTENT_COUNT`] always
//! show, higher desks appear only while occupied, the active desk is the
//! stock dot glyph, and empty desks are dimmed with pango alpha (a
//! single label cannot carry per-item CSS classes). "Occupied" means at
//! least one window on any of the desk's workspaces, across all
//! monitors - the aggregation waybar's own workspaces module cannot do.

use crate::model::DeskId;
use serde::Serialize;

/// Desks shown even when empty, mirroring Omarchy's stock
/// `persistent-workspaces` 1..=5.
const PERSISTENT_COUNT: u8 = 5;

/// The glyph Omarchy's stock bar uses for the active workspace.
const ACTIVE_ICON: &str = "\u{f14fb}";

/// The waybar custom-module JSON shape (`return-type: json`).
#[derive(Serialize)]
struct Payload {
    text: String,
    tooltip: String,
}

/// One waybar line for the given state. Pure; the daemon supplies live
/// occupancy.
pub fn status_line(current: DeskId, occupied: &[DeskId]) -> String {
    let persistent_max =
        DeskId::new(PERSISTENT_COUNT).expect("PERSISTENT_COUNT lies within 1..=DESK_COUNT");
    let mut items = Vec::new();
    for desk in DeskId::all() {
        if desk == current {
            items.push(ACTIVE_ICON.to_string());
        } else if occupied.contains(&desk) {
            items.push(label(desk));
        } else if desk <= persistent_max {
            items.push(format!("<span alpha='50%'>{}</span>", label(desk)));
        }
    }
    let payload = Payload {
        text: items.join(" "),
        tooltip: format!("Desk {current}"),
    };
    serde_json::to_string(&payload).expect("a payload of plain strings always serializes")
}

/// Single-glyph desk label, matching the stock bar (desk 10 renders "0").
fn label(desk: DeskId) -> String {
    if desk == DeskId::LAST {
        "0".to_string()
    } else {
        desk.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desk(id: u8) -> DeskId {
        DeskId::new(id).expect("valid desk id in test")
    }

    fn text_of(line: &str) -> String {
        let value: serde_json::Value = serde_json::from_str(line).expect("line is valid JSON");
        value["text"]
            .as_str()
            .expect("text is a string")
            .to_string()
    }

    fn dim(label: &str) -> String {
        format!("<span alpha='50%'>{label}</span>")
    }

    #[test]
    fn strip_shows_active_occupied_and_dimmed_persistent_desks() {
        let line = status_line(desk(3), &[desk(2), desk(3)]);
        let expected = format!("{} 2 {ACTIVE_ICON} {} {}", dim("1"), dim("4"), dim("5"));
        assert_eq!(text_of(&line), expected);
    }

    #[test]
    fn high_desks_appear_only_while_occupied_and_ten_renders_zero() {
        let with_ten = text_of(&status_line(desk(1), &[desk(10)]));
        assert!(with_ten.ends_with(" 0"), "text: {with_ten}");
        let without = text_of(&status_line(desk(1), &[]));
        assert!(!without.contains('7'), "text: {without}");
    }

    #[test]
    fn active_high_desk_extends_the_strip() {
        let text = text_of(&status_line(desk(8), &[]));
        assert!(text.ends_with(ACTIVE_ICON), "text: {text}");
    }

    #[test]
    fn tooltip_names_the_current_desk() {
        let line = status_line(desk(7), &[]);
        let value: serde_json::Value = serde_json::from_str(&line).expect("valid JSON");
        assert_eq!(value["tooltip"], "Desk 7");
    }
}
