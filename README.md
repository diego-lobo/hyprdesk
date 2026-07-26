# hyprdesk

Personal virtual-desktop system for Omarchy/Hyprland on this machine.

**Goal:** macOS / Windows / KDE-style shared-monitor desktops. One "desk"
spans ALL monitors (laptop eDP-1 + external DP-2 switch together), so
switching desks never leaves a monitor behind and there is no per-monitor
workspace hopping.

This is our own implementation, emulating the behavior of the upstream
`hyprland-virtual-desktops` C++ plugin, built for full custom control after
every packaged install path for the plugin failed on this system (AUR version
pin, hyprpm pin-table mismatch, root-owned state; full chain in
`docs/HANDOFF.md`).

## Layout

- `docs/HANDOFF.md` - why this project exists: complete failure chain from the
  install attempts, verified system facts, open cleanup items.
- `docs/DESIGN.md` - the behavior spec to emulate, architecture candidates
  with recommendation (IPC-based emulation vs C++ plugin), keybind plan,
  open design questions. Read this first when starting work.
- `reference/hyprland-virtual-desktops/` - upstream plugin clone (gitignored),
  pinned at `70a1ae6c` - the one commit PROVEN to compile against this
  system's Hyprland 0.55.4 headers. Includes the working `virtual-desktops.so`
  built 2026-07-26 as a fallback artifact. Re-fetch anytime with
  `scripts/fetch-reference.sh`.

## Status

Scaffolding only. No implementation yet, no system config touched. The
architecture pick in `docs/DESIGN.md` is a recommendation, not a decision -
red-team it (Codex) before building.
