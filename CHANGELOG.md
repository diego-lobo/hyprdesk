# Changelog

All notable changes to this project are documented here. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] - 2026-08-19

First public release.

### Added

- Shared-monitor desks: `vdesk`, `movetodesk`, `movetodesksilent`,
  `nextdesk`, `prevdesk`, `lastdesk`, `status`, `subscribe`, `waybar`.
- Resident daemon plus thin CLI client in a single binary, speaking a
  typed protocol over a per-session control socket.
- Stateless desk mapping: desk `d` on monitor slot `m` owns Hyprland
  workspace `d + 10*m`, pinned there by workspace rules.
- Monitor hotplug handling with window memory. Windows evacuated from a
  vanished monitor return to their original workspace when it comes back,
  which also covers the external monitor dropping during suspend.
- Desk tracking guards so Hyprland's own workspace juggling around a
  monitor (un)plug is not mistaken for a user desk switch.
- Quickshell bar widget (`hyprdesk.desks`) for Omarchy, showing desks
  rather than raw workspaces, with per-desk click targets and scroll.
- Waybar custom-module stream for non-Omarchy setups.
- `scripts/install.sh` and `scripts/uninstall.sh`: no sudo, fully
  reversible, symlinked config so updates cannot be silently reverted.

### Notes

Developed against Hyprland 0.56.2 and Omarchy 4.0.0 "Quattro". Requires
the Lua config engine introduced in Hyprland 0.55; the deprecated hyprlang
config format is not supported.

[Unreleased]: https://github.com/diego-lobo/hyprdesk/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/diego-lobo/hyprdesk/releases/tag/v1.0.0
