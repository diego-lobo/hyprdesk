# Project guidance for Claude Code - hyprdesk

Personal virtual-desktop system for THIS machine (Omarchy / Arch / Hyprland).
Read `docs/DESIGN.md` and `docs/HANDOFF.md` before doing anything.

## Hard constraints

- **NEVER modify anything in `~/.local/share/omarchy/`** (reading is fine and
  encouraged). It is git-managed by Omarchy; edits are lost on update and
  break the updater. All customization goes in `~/.config/`.
- **DO NOT touch any files inside `~/Projects/kriegsspiel`.** Unrelated
  project.
- **No sudo-dependent install or runtime paths.** The entire reason this
  project exists is that hyprpm's root-owned state machinery kept failing.
  Everything must build, install, load, and update from the user's home
  directory with user permissions only.
- After ANY Hyprland config change: `hyprctl reload` then
  `hyprctl configerrors`, and fix until clean.
- **Never self-verify visual outcomes with headless screenshots** - ask Diego
  to look.
- **No em dashes (U+2014) anywhere** - code, comments, docs, commit messages.
  Use an ASCII hyphen `-` or restructure.
- Commits: no trailers/footers ever (no Claude Code footer, no
  Co-Authored-By).

## System facts (verified 2026-07-26)

- Omarchy on Arch, Hyprland **0.55.4** (pacman `hyprland 0.55.4-1`, release
  tag commit `a0136d8c04687bb36eb8a28eb9d1ff92aea99704`).
- Arch's hyprland package ships full plugin dev headers
  (`/usr/include/hyprland/`, 526 files); `pkg-config --modversion hyprland`
  works. Plugins CAN be built against system headers with zero
  hyprpm/sudo involvement; pacman keeps headers in step with the compositor.
- Monitors: `eDP-1` (laptop) + `DP-2` (external). Treat monitor set as
  dynamic (hotplug) - never hardcode.
- Omarchy binds the number row by KEYCODE (`code:10`..`code:19`), with
  `bindd` (description field). Defaults load from
  `~/.local/share/omarchy/default/hypr/bindings/tiling-v2.conf` BEFORE
  `~/.config/hypr/bindings.conf`, so overriding a default bind requires an
  `unbind` line first.
- User Hyprland config entry point: `~/.config/hypr/hyprland.conf`; our
  additions should live in a single new sourced file (one-line revert).
- `sudo` prompts for a password and Claude's shell has no TTY; anything
  needing sudo must be handed to Diego to run in a real terminal (suggest the
  `! command` prefix).

## Decision policy

- Settle architecture/library/design picks with an independent Codex
  red-team, then decide autonomously on the merits (do not defer
  automatically, do not AskUserQuestion for those). Pattern:
  `codex exec --sandbox read-only -c tools.web_search=true "..." < /dev/null`
  (needs Bash sandbox off + a timeout; codex hangs reading stdin otherwise).
  Make Codex investigate independently; do not feed it our conclusions.
- Never contrive: if an approach fails, report the finding; do not paper over
  it with a workaround that hides the failure.

## Reference material

- `reference/hyprland-virtual-desktops/` (gitignored) = upstream plugin at
  commit `70a1ae6c`, the exact behavioral reference to emulate, INCLUDING a
  working `virtual-desktops.so` compiled 2026-07-26 against this system's
  headers (fallback artifact). `scripts/fetch-reference.sh` re-clones it.
- Codebase is small: 1,486 LOC across 11 C++ files (main.cpp 489,
  VirtualDeskManager.cpp 317, VirtualDesk.cpp 192, utils, sticky_apps,
  lua_bindings + headers). Extract exact dispatcher semantics from source,
  not from memory.
