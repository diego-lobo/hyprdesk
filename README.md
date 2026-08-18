# hyprdesk

Personal virtual-desktop system for Omarchy/Hyprland on this machine.

**Goal:** macOS / Windows / KDE-style shared-monitor desktops. One "desk"
spans ALL monitors (laptop and external switch together), so switching
desks never leaves a monitor behind and there is no per-monitor workspace
hopping.

This is our own implementation, emulating the behavior of the upstream
`hyprland-virtual-desktops` C++ plugin, built for full custom control after
every packaged install path for the plugin failed on this system (AUR version
pin, hyprpm pin-table mismatch, root-owned state; full chain in
`docs/HANDOFF.md`).

## Layout

- `config/` - the system integration files, symlinked into `~/.config`
  (see below).
- `docs/HANDOFF.md` - why this project exists: complete failure chain from the
  install attempts, and the pre-Quattro system facts (historical).
- `docs/DESIGN.md` - the behavior spec to emulate, architecture candidates
  with recommendation (IPC-based emulation vs C++ plugin), keybind plan,
  open design questions. Read this first when starting work.
- `reference/hyprland-virtual-desktops/` - upstream plugin clone (gitignored),
  pinned at `70a1ae6c` - the one commit proven to compile against Hyprland
  0.55.4 headers. Includes the `virtual-desktops.so` built 2026-07-26 as a
  fallback artifact; that build predates the 0.56 upgrade and has not been
  revalidated against it. Re-fetch anytime with `scripts/fetch-reference.sh`.

## How it runs

A resident daemon owns desk state and drives the compositor over its
public request socket; a thin CLI client in the same binary talks to the
daemon. Desk `d` on the monitor in slot `m` is Hyprland workspace
`d + 10*m`, and workspace rules pin each workspace to its monitor.

    hyprdesk daemon                 # started by the Hyprland config
    hyprdesk vdesk 3                # switch every monitor to desk 3
    hyprdesk movetodesk 3           # move the active window there and follow
    hyprdesk movetodesksilent 3     # ...without following
    hyprdesk nextdesk|prevdesk [--cycle]
    hyprdesk lastdesk               # back-and-forth
    hyprdesk status [--json]

System integration lives in `config/` and is SYMLINKED into place, so the
repo is the source of truth and an `omarchy update` cannot quietly revert
it:

    ~/.config/hypr/hyprdesk.lua            -> config/hypr/hyprdesk.lua
    ~/.config/omarchy/plugins/diego.desks  -> config/omarchy/plugins/diego.desks

- `config/hypr/hyprdesk.lua` - keybinds and daemon autostart. Activated by
  one `require("hypr.hyprdesk")` line in `~/.config/hypr/hyprland.lua`;
  deleting that line and the symlink is a full revert.
- `config/omarchy/plugins/diego.desks/` - Quickshell bar widget drawing the
  desk strip. Enabled with `omarchy plugin enable diego.desks`, which also
  takes the slot vacated by `omarchy plugin disable omarchy.workspaces`.

## Status

Implemented and in daily use. Ported 2026-08-18 to Omarchy 4 "Quattro"
(Hyprland 0.56.2), which replaced the compositor's config/action grammar
with Lua, moved Omarchy's Hyprland config from `.conf` to `.lua`, and
dropped waybar for a Quickshell bar. See the port section of
`docs/DESIGN.md` for the verified action grammar and the four integration
points to re-check after any future update.
