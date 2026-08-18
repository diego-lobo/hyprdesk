# Project guidance for Claude Code - hyprdesk

Personal virtual-desktop system for THIS machine (Omarchy / Arch / Hyprland).
Read `docs/DESIGN.md` and `docs/HANDOFF.md` before doing anything.

## Hard constraints

- **NEVER modify anything in `/usr/share/omarchy/`** (reading is fine and
  encouraged). It is owned by the `omarchy` package; edits are lost on
  update. All customization goes in `~/.config/`. (Pre-Quattro this tree
  lived at `~/.local/share/omarchy/`; a stale copy may still be there.)
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
- **Code standard (hard rule):** all code and structural design must be
  professional review-grade work, the bar being software for the
  military/Air Force/DoD: strict idiomatic Rust design patterns, human
  readability and ease of understanding/extensibility first, and elegant
  simplicity and modularity that an experienced senior engineer would sign
  off on. No `unsafe`. Typed errors, no stringly-typed protocol surfaces,
  documented invariants, tests for pure logic.
- Commits: no trailers/footers ever (no Claude Code footer, no
  Co-Authored-By).

## System facts (verified 2026-08-18, after the Omarchy 4 upgrade)

- Omarchy **4.0.0 "Quattro"** on Arch, Hyprland **0.56.2** (pacman
  `omarchy 4.0.0-1`, `hyprland 0.56.2-1`, tag commit `efb50993`).
- Omarchy now installs to **`/usr/share/omarchy/`** (package-owned,
  READ-ONLY, reading encouraged), not `~/.local/share/omarchy/`.
- Hyprland's config engine is **Lua**. The legacy text grammar is gone:
  `keyword` is rejected outright ("Use eval.") and `dispatch` args parse
  as Lua. Drive the compositor with `eval <lua chunk>`; `[[BATCH]]` is
  retired. `hyprdesk` speaks this through the typed `hypr::Command` enum
  - see the verified grammar table in `docs/DESIGN.md`.
- User Hyprland config entry point: **`~/.config/hypr/hyprland.lua`**
  (confirmed in the compositor log). The old `~/.config/hypr/*.conf`
  files survive on disk from the migration but are NEVER read - do not be
  fooled by them. Ours lives in `~/.config/hypr/hyprdesk.lua`, required
  from `hyprland.lua` (one-line revert).
- Overriding a default bind still needs an unbind first, now
  `hl.unbind("SUPER + code:10")` before `o.bind(...)`; defaults come from
  `/usr/share/omarchy/default/hypr/bindings/tiling.lua`. The number row
  is still KEYCODE-based (`code:10`..`code:19` = 1..0). Note
  `hyprctl binds` does NOT populate the key/keycode fields for these -
  match on `description`.
- **Waybar is gone.** The bar is Quickshell (`omarchy-shell`) with plugin
  widgets; user plugins live in `~/.config/omarchy/plugins/<id>/`. Ours is
  `hyprdesk.desks`. QML errors land in `journalctl --user`.
- Arch's hyprland package still ships plugin dev headers
  (`/usr/include/hyprland/`) and `pkg-config --modversion hyprland` works,
  so the plugin route remains technically open; the 0.55.4-era
  `virtual-desktops.so` in `reference/` has NOT been revalidated on 0.56.
- Monitors on this machine are currently `eDP-1` (laptop) + `HDMI-A-1`
  (external; it was `DP-2` before). Treat the monitor set as dynamic
  (hotplug) - never hardcode.
- `sudo` prompts for a password and Claude's shell has no TTY; anything
  needing sudo must be handed to Diego to run in a real terminal (suggest the
  `! command` prefix).

## System integration is symlinked from `config/`

`~/.config/hypr/hyprdesk.lua` and `~/.config/omarchy/plugins/hyprdesk.desks`
are SYMLINKS into this repo's `config/` tree. Edit the repo copies; never
replace a symlink with a regular file.

## Decision policy

- Settle architecture/library/design picks autonomously on the merits (do
  not defer automatically, do not AskUserQuestion for those).
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
